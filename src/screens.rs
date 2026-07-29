use serde::Serialize;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::EnumDisplayMonitors;

#[derive(Debug, Clone, Serialize)]
pub struct ScreenInfo {
    pub index: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

/// 枚举所有显示器，按从左到右排序并分配编号（0, 1, 2...）
pub fn enumerate_screens() -> Vec<ScreenInfo> {
    let mut rects: Vec<RECT> = Vec::new();

    unsafe {
        let ctx = &mut rects as *mut Vec<RECT> as isize;

        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(ctx),
        );
    }

    // 按「从上到下、从左到右」排序，分配编号
    // 先按 top 排（上面的屏幕优先），再按 left 排（同行的左边优先）
    rects.sort_by(|a, b| {
        a.top
            .cmp(&b.top)
            .then_with(|| a.left.cmp(&b.left))
    });
    let mut screens: Vec<ScreenInfo> = Vec::new();
    for (i, rect) in rects.into_iter().enumerate() {
        screens.push(ScreenInfo {
            index: i as i32,
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        });
    }

    screens
}

unsafe extern "system" fn monitor_enum_proc(
    _hmon: windows::Win32::Graphics::Gdi::HMONITOR,
    _hdc: windows::Win32::Graphics::Gdi::HDC,
    lprc_monitor: *mut RECT,
    dw_data: LPARAM,
) -> BOOL {
    if lprc_monitor.is_null() {
        return BOOL(1);
    }
    let rect = *lprc_monitor;
    let rects = &mut *(dw_data.0 as *mut Vec<RECT>);
    rects.push(rect);
    BOOL(1) // continue enumeration
}
