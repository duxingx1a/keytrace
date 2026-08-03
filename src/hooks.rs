use crate::screens::{self, ScreenInfo};
use serde::{Serialize, Deserialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use windows::Win32::Foundation::{BOOL, HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::Threading::GetCurrentThreadId;

/// 鼠标按键枚举
/// 数据库中存字符串："left" / "right" / "middle" / "x1" / "x2"
/// 代码中用枚举，省内存（1 字节 vs String 的 24+ 字节）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

impl MouseButton {
    /// 转为数据库存储用的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            MouseButton::Left => "left",
            MouseButton::Right => "right",
            MouseButton::Middle => "middle",
            MouseButton::X1 => "x1",
            MouseButton::X2 => "x2",
        }
    }

    /// 从 Windows 消息 ID 解析按键，可选 wParam 用于区分 X1/X2
    pub fn from_msg_id(msg_id: u32, wparam: usize) -> Option<MouseButton> {
        match msg_id {
            WM_LBUTTONDOWN | WM_LBUTTONUP => Some(MouseButton::Left),
            WM_RBUTTONDOWN | WM_RBUTTONUP => Some(MouseButton::Right),
            WM_MBUTTONDOWN | WM_MBUTTONUP => Some(MouseButton::Middle),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                if (wparam >> 16) & 0xFFFF == 2 { Some(MouseButton::X2) }
                else { Some(MouseButton::X1) }
            }
            _ => None,
        }
    }
}

/// 鼠标移动事件（从钩子线程发给处理线程）
#[derive(Debug, Clone)]
pub struct MouseMoveEvent {
    pub ts: i64,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub screen_index: i32,
}

/// 键盘按键事件
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key_code: u32,
}

#[derive(Debug, Clone)]
pub enum HookEvent {
    Key(KeyEvent),
    MouseMove(MouseMoveEvent),
    MouseClick {
        ts: i64,
        x: i32,
        y: i32,
        button: MouseButton,
        w: i32,
        h: i32,
        screen_index: i32,
    },
}

static RUNNING: AtomicBool = AtomicBool::new(true);

// 缓存屏幕列表，供 mouse_proc 判断坐标所在屏幕
// 用 Mutex 保护，避免 static mut 的悬垂引用/UB 风险
static SCREENS: Mutex<Vec<ScreenInfo>> = Mutex::new(Vec::new());

/// 根据坐标找到所在屏幕，返回 (index, width, height)
fn find_screen(x: i32, y: i32) -> Option<(i32, i32, i32)> {
    if let Ok(screens) = SCREENS.lock() {
        for s in screens.iter() {
            if x >= s.left && x < s.right && y >= s.top && y < s.bottom {
                return Some((s.index, s.width, s.height));
            }
        }
    }
    None
}

/// 启动钩子，通过 channel 发送事件，不阻塞系统消息链
pub fn run_hooks(tx: Sender<HookEvent>) {
    // 枚举屏幕，缓存到 static
    if let Ok(mut screens) = SCREENS.lock() {
        *screens = screens::enumerate_screens();
    }

    let instance = HINSTANCE(std::ptr::null_mut());

    let kb_hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            HINSTANCE(instance.0),
            0,
        )
    };
    let kb_hook = kb_hook.expect("WH_KEYBOARD_LL 钩子注册失败");

    let mouse_hook =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE(instance.0), 0) };
    let mouse_hook = mouse_hook.expect("WH_MOUSE_LL 钩子注册失败");

    // 用 Mutex 包一下 channel sender 方便在 extern "system" 回调中用
    unsafe {
        CHANNEL = Some(Mutex::new(tx));
    }

    let tid = unsafe { GetCurrentThreadId() };
    HOOK_THREAD_ID.store(tid, Ordering::Relaxed);

    let mut msg = MSG::default();
    while RUNNING.load(Ordering::Relaxed) {
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret == BOOL(0) {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = UnhookWindowsHookEx(kb_hook);
        let _ = UnhookWindowsHookEx(mouse_hook);
    }
}

static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// 唤醒钩子线程的消息循环（在退出时调用）
pub fn wake_hook_thread() {
    let tid = HOOK_THREAD_ID.load(Ordering::Relaxed);
    if tid != 0 {
        unsafe { let _ = PostThreadMessageW(tid, WM_NULL, WPARAM(0), LPARAM(0)); }
    }
}

/// 停止钩子消息循环，等待线程自然退出并清理钩子资源
pub fn stop_hooks() {
    RUNNING.store(false, Ordering::Relaxed);
    wake_hook_thread();
}

// 全局 channel
static mut CHANNEL: Option<Mutex<Sender<HookEvent>>> = None;

/// 向 channel 发送事件（非阻塞）
fn send_event(event: HookEvent) {
    unsafe {
        if let Some(ref ch) = CHANNEL {
            if let Ok(lock) = ch.lock() {
                let _ = lock.send(event);
            }
        }
    }
}

/// 键盘钩子过程
unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg_id = wparam.0 as u32;
        let is_down = msg_id == WM_KEYDOWN || msg_id == WM_SYSKEYDOWN;
        if is_down {
            let kbd_ptr = lparam.0 as *const KBDLLHOOKSTRUCT;
            if !kbd_ptr.is_null() {
                let kbd = *kbd_ptr;
                let mut key_code = kbd.vkCode;
                // LLKHF_EXTENDED (0x01): 区分扩展键（小键盘 Enter 等）
                // 扩展键的 key_code 加 0x100 偏移，避免与主键盘区同名键冲突
                if (kbd.flags.0 & 0x01) != 0 {
                    key_code += 0x100;
                }
                send_event(HookEvent::Key(KeyEvent {
                    key_code,
                }));
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// 鼠标钩子过程
/// 注意：用上次移动时间做采样间隔 + 位置变化检测
static mut LAST_MOUSE_TS: i64 = 0;
static mut LAST_MOUSE_X: i32 = i32::MAX;
static mut LAST_MOUSE_Y: i32 = i32::MAX;

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let ms_ptr = lparam.0 as *const MSLLHOOKSTRUCT;
        if !ms_ptr.is_null() {
            let ms = *ms_ptr;
            let msg_id = wparam.0 as u32;

            if msg_id == WM_MOUSEMOVE {
                // 位置没变 → 忽略（防传感器抖动）
                if ms.pt.x == LAST_MOUSE_X && ms.pt.y == LAST_MOUSE_Y {
                    return CallNextHookEx(None, code, wparam, lparam);
                }
                LAST_MOUSE_X = ms.pt.x;
                LAST_MOUSE_Y = ms.pt.y;

                // 采样间隔：50ms
                let now_ms = chrono::Local::now().timestamp_millis();
                if now_ms - LAST_MOUSE_TS >= 50 {
                    LAST_MOUSE_TS = now_ms;
                    let screen = find_screen(ms.pt.x, ms.pt.y);
                    let (screen_index, w, h) = screen.unwrap_or((-1, 0, 0));
                    send_event(HookEvent::MouseMove(MouseMoveEvent {
                        ts: now_ms,
                        x: ms.pt.x,
                        y: ms.pt.y,
                        w,
                        h,
                        screen_index,
                    }));
                }
            } else {
                let btn = MouseButton::from_msg_id(msg_id, wparam.0);
                if let Some(button) = btn {
                    let ts = chrono::Local::now().timestamp_millis();
                    let screen = find_screen(ms.pt.x, ms.pt.y);
                    let (screen_index, w, h) = screen.unwrap_or((-1, 0, 0));
                    send_event(HookEvent::MouseClick {
                        ts,
                        x: ms.pt.x,
                        y: ms.pt.y,
                        button,
                        w,
                        h,
                        screen_index,
                    });
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
