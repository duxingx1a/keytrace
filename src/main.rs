#![windows_subsystem = "windows"]

mod api;
mod config;
mod db;
mod hooks;
mod processor;
mod screens;
mod session;
mod static_files;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::io::Write;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};
use windows::Win32::System::Threading::{OpenProcess, GetExitCodeProcess, PROCESS_QUERY_INFORMATION};
use windows::Win32::Foundation::CloseHandle;

/// STILL_ACTIVE：进程仍在运行的退出码（259）
const STILL_ACTIVE: u32 = 259;

/// 退出时自动删除锁文件
struct LockGuard(std::path::PathBuf);
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// 检查 PID 对应的进程是否仍在运行
/// 用 GetExitCodeProcess 判断：进程已退出时返回非 STILL_ACTIVE 的退出码
/// （仅用 OpenProcess 不可靠——已死进程可能仍能打开句柄）
fn is_process_running(pid: u32) -> bool {
    if pid == 0 { return false; }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
        if handle.is_err() { return false; }
        let handle = handle.unwrap();
        let mut exit_code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut exit_code).is_ok();
        let _ = CloseHandle(handle);
        // 打开失败或退出码不是 STILL_ACTIVE → 进程已死
        ok && exit_code == STILL_ACTIVE
    }
}

fn main() {
    // ── 单实例检测（PID 文件锁）──
    let lock_path = std::env::temp_dir().join("keytrace_instance.lock");
    let mut _lock_file = None;
    let mut lock_created = false;

    // 尝试以独占方式创建锁文件（create_new：文件已存在则失败）
    if let Ok(f) = std::fs::OpenOptions::new().write(true).create_new(true).open(&lock_path) {
        _lock_file = Some(f);
        lock_created = true;
    } else {
        // 锁文件已存在 — 读取 PID，检查进程是否还活着
        let mut stale = false;
        if let Ok(content) = std::fs::read_to_string(&lock_path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if !is_process_running(pid) {
                    stale = true;
                }
            } else {
                // 内容不是合法 PID，视为僵尸锁
                stale = true;
            }
        } else {
            // 读不到内容，视为僵尸锁
            stale = true;
        }

        if stale {
            // 僵尸锁：直接覆盖写入（truncate），避免「先删再建」的竞态
            if let Ok(f) = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&lock_path)
            {
                _lock_file = Some(f);
                lock_created = true;
            }
        }
    }

    if !lock_created {
        unsafe { MessageBoxW(None, w!("KeyTrace 已在运行中"), w!("KeyTrace"), MB_OK); }
        return;
    }

    // 写入当前 PID
    if let Some(ref mut f) = _lock_file {
        let _ = writeln!(f, "{}", std::process::id());
    }
    let _lock_guard = LockGuard(lock_path);

    println!("[keytrace] 启动中...");

    let args: Vec<String> = std::env::args().collect();
    let safe_mode = args.iter().any(|a| a == "--safe");

    let config = config::Config::load();
    println!("[keytrace] 端口: {}", config.port);
    println!("[keytrace] DB: {}", config.db_path.display());
    println!("[keytrace] 空闲超时: {}ms", config.idle_timeout_ms);

    let db = Arc::new(db::Database::open(&config.db_path).expect("数据库初始化失败"));

    if config.retention_days > 0 {
        if let Err(e) = db.cleanup_old_data(config.retention_days) {
            eprintln!("[keytrace] 清理旧数据失败: {}", e);
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let start_time = Instant::now();

    // ---- 创建 channel：钩子线程 → 处理线程 ----
    let (tx, rx) = mpsc::channel::<hooks::HookEvent>();

    // ---- 会话管理器 ----
    let session_mgr = Arc::new(std::sync::Mutex::new(session::SessionManager::new(
        db.clone(),
        config.idle_timeout_ms,
    )));

    // ---- 启动事件处理线程（内存缓冲区 + 每分钟 flush） ----
    let db_proc = db.clone();
    let sm_proc = session_mgr.clone();
    let running_proc = running.clone();
    thread::spawn(move || {
        processor::start_event_processor(rx, db_proc, sm_proc, running_proc);
    });

    // ---- 启动 HTTP API 线程 ----
    let db_api = db.clone();
    let running_api = running.clone();
    let config_api = config::Config {
        port: config.port,
        ..config::Config::default()
    };
    thread::spawn(move || {
        api::start_api(db_api, &config_api, running_api, start_time);
    });

    // ---- 启动空闲检测线程 ----
    let running_idle = running.clone();
    let sm_idle = session_mgr.clone();
    thread::spawn(move || {
        while running_idle.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            if let Ok(mut sm) = sm_idle.lock() {
                sm.check_idle();
            }
        }
    });

    // ---- 启动数据清理线程 ----
    let running_clean = running.clone();
    let db_clean = db.clone();
    let retention = config.retention_days;
    thread::spawn(move || {
        while running_clean.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(3600));
            if retention > 0 {
                if let Err(e) = db_clean.cleanup_old_data(retention) {
                    eprintln!("[keytrace] 定期清理失败: {}", e);
                }
            }
        }
    });

    // ---- 启动托盘图标线程（主循环寄托于此） ----
    tray::start_tray(running.clone());

    if safe_mode {
        println!("[keytrace] ⚠️ 安全模式：不安装键盘/鼠标钩子");
        println!("[keytrace] 仅启动 API 服务器 (已最小化到托盘)");
    } else {
        println!("[keytrace] 已启动（已最小化到托盘）。右键托盘图标打开控制面板");
        // 钩子在独立线程运行，不阻塞
        thread::spawn(move || {
            hooks::run_hooks(tx);
        });
    }

    // 主线程等待托盘退出信号
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
    }

    // ---- 清理退出 ----
    running.store(false, Ordering::Relaxed);

    if let Ok(mut sm) = session_mgr.lock() {
        sm.check_idle();
    }

    println!("[keytrace] 已退出");
}
