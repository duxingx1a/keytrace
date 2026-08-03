use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP,
    NIIF_INFO, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DispatchMessageW, GetCursorPos, GetMenuItemRect, GetMessageW,
    HMENU, LoadIconW, ModifyMenuW, MF_BYPOSITION, MF_SEPARATOR, MF_STRING,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    TrackPopupMenu, TPM_BOTTOMALIGN, TPM_LEFTALIGN, CW_USEDEFAULT,
    IDI_APPLICATION, SW_SHOW, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK,
    WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

const TRAY_ICON_ID: u32 = 1;
const WM_TRAY: u32 = WM_USER + 100;
const CMD_OPEN: usize = 1001;
const CMD_QUIT: usize = 1002;
const CMD_AUTOSTART: usize = 1003;
const CMD_ABOUT: usize = 1004;

// explorer 重启后广播的 TaskbarCreated 消息 ID（运行时注册）
static mut TASKBAR_CREATED_MSG: u32 = 0;

// 全局 running 标志，供 wnd_proc 在退出时置 false
static mut RUNNING_FLAG: Option<Arc<AtomicBool>> = None;

unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TRAY => {
            let event = lparam.0 as u32 & 0xFFFF;
            if event == WM_RBUTTONUP as u32 {
                handle_tray_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK as u32 {
                open_dashboard();
            }
            LRESULT(0)
        }
        _ if msg == unsafe { TASKBAR_CREATED_MSG } => {
            // explorer 重启后托盘区被清空，重新注册图标
            create_tray(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = wparam.0 as usize;
            if cmd == CMD_OPEN {
                open_dashboard();
            } else if cmd == CMD_AUTOSTART {
                let enabled = !is_autostart_enabled();
                set_autostart(enabled);
            } else if cmd == CMD_QUIT {
                cleanup_tray(hwnd);
                unsafe {
                    if let Some(ref flag) = RUNNING_FLAG {
                        flag.store(false, Ordering::Relaxed);
                    }
                }
                // 唤醒钩子线程的 GetMessageW，让它自然退出并清理钩子
                crate::hooks::stop_hooks();
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn cleanup_tray(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    unsafe { let _ = Shell_NotifyIconW(NIM_DELETE, &nid); }
}

fn load_custom_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    let ico_data = include_bytes!("../keytrace.ico");
    let offset = unsafe {
        windows::Win32::UI::WindowsAndMessaging::LookupIconIdFromDirectoryEx(
            ico_data.as_ptr() as *const _,
            true,
            32, 32,
            windows::Win32::UI::WindowsAndMessaging::LR_DEFAULTCOLOR,
        )
    };
    if offset > 0 {
        if let Ok(handle) = unsafe {
            windows::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx(
                std::slice::from_raw_parts(
                    ico_data.as_ptr().add(offset as usize),
                    ico_data.len() - offset as usize,
                ),
                true,
                0x00030000,
                0, 0,
                windows::Win32::UI::WindowsAndMessaging::LR_DEFAULTCOLOR,
            )
        } {
            if !handle.is_invalid() {
                return windows::Win32::UI::WindowsAndMessaging::HICON(handle.0);
            }
        }
    }
    unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default()
}

fn create_tray(hwnd: HWND) {
    let h_icon = load_custom_icon();

    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = h_icon;
    let tip: Vec<u16> = "KeyTrace\0".encode_utf16().collect();
    let tip_len = tip.len().min(128);
    nid.szTip[..tip_len].copy_from_slice(&tip[..tip_len]);

    unsafe { let _ = Shell_NotifyIconW(NIM_ADD, &nid); }
}

const RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn is_autostart_enabled() -> bool {
    let output = std::process::Command::new("reg")
        .args(["query", RUN_KEY, "/v", "KeyTrace"])
        .output();
    output.map(|o| o.status.success()).unwrap_or(false)
}

fn set_autostart(enable: bool) {
    let exe = std::env::current_exe().unwrap_or_default();
    let path = exe.to_string_lossy();
    if enable {
        let _ = std::process::Command::new("reg")
            .args(["add", RUN_KEY, "/v", "KeyTrace", "/t", "REG_SZ", "/d", &path, "/f"])
            .output();
    } else {
        let _ = std::process::Command::new("reg")
            .args(["delete", RUN_KEY, "/v", "KeyTrace", "/f"])
            .output();
    }
}

fn open_dashboard() {
    unsafe {
        let port = crate::api::get_actual_port();
        let actual = if port > 0 { port } else { 50555 };
        let url_str = format!("http://localhost:{}", actual);
        let url_wide: Vec<u16> = url_str.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = ShellExecuteW(
            None,
            w!("open"),
            PCWSTR::from_raw(url_wide.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

fn handle_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        let _ = AppendMenuW(menu, MF_STRING, CMD_OPEN, w!("打开 Dashboard"));
        let auto_label = if is_autostart_enabled() { w!("✓ 开机自启动") } else { w!("  开机自启动") };
        let _ = AppendMenuW(menu, MF_STRING, CMD_AUTOSTART, auto_label);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("退出"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_ABOUT, w!("关于 KeyTrace"));
        let _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        // 悬停检测线程：鼠标悬停在「关于」项上时，菜单文本换成 exe 绝对路径，移开后还原
        let menu_open = Arc::new(AtomicBool::new(true));
        let menu_open_hover = menu_open.clone();
        // HWND/HMENU 是裸指针（非 Send），转 usize 传入线程，在线程内还原
        let hwnd_val = hwnd.0 as usize;
        let menu_val = menu.0 as usize;
        let hover_thread = thread::spawn(move || {
            let hwnd = HWND(hwnd_val as *mut core::ffi::c_void);
            let menu = HMENU(menu_val as *mut core::ffi::c_void);
            let about_pos = 5u32; // 菜单项位置：0=打开 1=自启动 2=分隔 3=退出 4=分隔 5=关于
            let exe_path = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut showing = false;
            while menu_open_hover.load(Ordering::Relaxed) {
                let mut rect: RECT = std::mem::zeroed();
                if GetMenuItemRect(hwnd, menu, about_pos, &mut rect).is_ok() {
                    let mut cursor = POINT::default();
                    let _ = GetCursorPos(&mut cursor);
                    let hover = cursor.x >= rect.left && cursor.x <= rect.right
                        && cursor.y >= rect.top && cursor.y <= rect.bottom;
                    if hover && !showing {
                        let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
                        let _ = ModifyMenuW(menu, about_pos, MF_BYPOSITION | MF_STRING, CMD_ABOUT, PCWSTR::from_raw(wide.as_ptr()));
                        showing = true;
                    } else if !hover && showing {
                        let _ = ModifyMenuW(menu, about_pos, MF_BYPOSITION | MF_STRING, CMD_ABOUT, w!("关于 KeyTrace"));
                        showing = false;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
        menu_open.store(false, Ordering::Relaxed);
        let _ = hover_thread.join();

        let _ = DestroyMenu(menu);
    }
}

fn show_balloon(hwnd: HWND, title: &str, msg: &str) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    let t: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let m: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    let tl = t.len().min(64); let ml = m.len().min(256);
    nid.szInfoTitle[..tl].copy_from_slice(&t[..tl]);
    nid.szInfo[..ml].copy_from_slice(&m[..ml]);
    unsafe { let _ = Shell_NotifyIconW(NIM_MODIFY, &nid); }
}

pub fn start_tray(running: Arc<AtomicBool>) {
    unsafe { RUNNING_FLAG = Some(running.clone()); }
    thread::spawn(move || {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

        let h_instance: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap().into() };

        let class_name = w!("KeyTraceTray");

        // 注册 explorer 重启通知消息（必须在窗口创建前注册）
        unsafe {
            TASKBAR_CREATED_MSG = RegisterWindowMessageW(w!("TaskbarCreated"));
        }

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance,
            lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
            ..unsafe { std::mem::zeroed() }
        };

        let atom = unsafe { RegisterClassW(&wc) };
        if atom == 0 {
            eprintln!("[keytrace] 托盘窗口类注册失败");
            return;
        }

        let hwnd = match unsafe {
            CreateWindowExW(
                Default::default(),
                PCWSTR::from_raw(class_name.as_ptr()),
                w!("KeyTraceTray"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                h_instance,
                None,
            )
        } {
            Ok(h) => h,
            Err(_) => {
                eprintln!("[keytrace] 托盘窗口创建失败");
                return;
            }
        };

        create_tray(hwnd);
        show_balloon(hwnd, "KeyTrace", "已启动，正在后台运行");

        println!("[keytrace] 托盘图标已创建。右键打开控制面板");

        let mut msg: windows::Win32::UI::WindowsAndMessaging::MSG = unsafe { std::mem::zeroed() };
        while running.load(Ordering::Relaxed) {
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if ret.0 <= 0 {
                break;
            }
            unsafe {
                DispatchMessageW(&msg);
            }
        }

        cleanup_tray(hwnd);
        println!("[keytrace] 托盘线程已退出");
    });

    thread::sleep(Duration::from_millis(200));
}
