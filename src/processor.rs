use crate::db::Database;
use chrono::Timelike;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 事件处理线程：接收 channel 事件 → 内存累计 → 每 60s flush 到 SQLite
pub fn start_event_processor(
    rx: Receiver<crate::hooks::HookEvent>,
    db: Arc<Database>,
    session_mgr: Arc<std::sync::Mutex<crate::session::SessionManager>>,
    running: Arc<AtomicBool>,
) {
    // 内存缓冲区
    let mut buf_keys: Vec<(String, u32, u32, i64)> = Vec::new();
    let mut buf_moves: Vec<(i64, i32, i32, i32, i32, i32)> = Vec::new();
    let mut buf_clicks: Vec<(i64, i32, i32, crate::hooks::MouseButton, i32, i32, i32)> = Vec::new();

    // 鼠标移动采样二次限制
    let mut last_move_ts: i64 = 0;
    let mouse_move_interval_ms: i64 = 100;

    // Flush 计时
    let mut last_flush = Instant::now();
    let flush_interval = Duration::from_secs(60);

    let mut pending_keys: usize = 0;
    let mut pending_moves: usize = 0;
    let mut pending_clicks: usize = 0;

    'proc: while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                match event {
                    crate::hooks::HookEvent::Key(ke) => {
                        let now = chrono::Local::now();
                        let date = now.format("%Y-%m-%d").to_string();
                        let hour = now.hour();
                        buf_keys.push((date, hour, ke.key_code, 1));
                        pending_keys += 1;

                        // 活跃会话即时更新
                        if let Ok(mut sm) = session_mgr.lock() {
                            sm.on_key(now.timestamp_millis());
                        }
                    }
                    crate::hooks::HookEvent::MouseMove(me) => {
                        if me.ts - last_move_ts >= mouse_move_interval_ms {
                            last_move_ts = me.ts;
                            buf_moves.push((me.ts, me.x, me.y, me.w, me.h, me.screen_index));
                            pending_moves += 1;
                        }
                    }
                    crate::hooks::HookEvent::MouseClick {
                        ts,
                        x,
                        y,
                        button,
                        w,
                        h,
                        screen_index,
                    } => {
                        buf_clicks.push((ts, x, y, button, w, h, screen_index));
                        pending_clicks += 1;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break 'proc,
        }

        // 每 60s 或缓冲区超过阈值时 flush
        let elapsed = last_flush.elapsed();
        let buf_full = pending_keys > 5000 || pending_moves > 5000 || pending_clicks > 1000;

        if elapsed >= flush_interval || buf_full {
            do_flush(
                &db,
                &mut buf_keys,
                &mut buf_moves,
                &mut buf_clicks,
                &mut pending_keys,
                &mut pending_moves,
                &mut pending_clicks,
            );
            last_flush = Instant::now();
        }
    }

    // 退出前 flush 剩余数据
    do_flush(
        &db,
        &mut buf_keys,
        &mut buf_moves,
        &mut buf_clicks,
        &mut pending_keys,
        &mut pending_moves,
        &mut pending_clicks,
    );
}

fn do_flush(
    db: &Database,
    buf_keys: &mut Vec<(String, u32, u32, i64)>,
    buf_moves: &mut Vec<(i64, i32, i32, i32, i32, i32)>,
    buf_clicks: &mut Vec<(i64, i32, i32, crate::hooks::MouseButton, i32, i32, i32)>,
    pending_keys: &mut usize,
    pending_moves: &mut usize,
    pending_clicks: &mut usize,
) {
    if *pending_keys == 0 && *pending_moves == 0 && *pending_clicks == 0 {
        return;
    }

    // 合并按键
    if !buf_keys.is_empty() {
        let mut merged: HashMap<(String, u32, u32), i64> = HashMap::new();
        let mut hourly_key_counts: HashMap<(String, u32), i64> = HashMap::new();
        for (d, h, k, c) in buf_keys.drain(..) {
            *merged.entry((d.clone(), h, k)).or_insert(0) += c;
            *hourly_key_counts.entry((d, h)).or_insert(0) += c;
        }
        for ((d, h, k), c) in merged {
            let _ = db.record_key_batch(&d, h, k, c);
        }
        for ((d, h), c) in hourly_key_counts {
            let _ = db.upsert_hourly_keys(&d, h, c);
        }
    }

    // 批量写鼠标移动 + 小时聚合
    if !buf_moves.is_empty() {
        let mut hourly_moves: HashMap<(String, u32), i64> = HashMap::new();
        for m in buf_moves.iter() {
            let dt = chrono::DateTime::from_timestamp_millis(m.0).unwrap_or_default();
            *hourly_moves.entry((dt.format("%Y-%m-%d").to_string(), dt.hour())).or_insert(0) += 1;
        }
        let batch: Vec<_> = buf_moves.drain(..).collect();
        let _ = db.record_mouse_moves_batch(&batch);
        for ((d, h), c) in hourly_moves {
            let _ = db.upsert_hourly_mouse_moves(&d, h, c);
        }
    }

    // 批量写鼠标点击 + 小时聚合
    if !buf_clicks.is_empty() {
        let mut hourly_clicks: HashMap<(String, u32), i64> = HashMap::new();
        for c in buf_clicks.iter() {
            let dt = chrono::DateTime::from_timestamp_millis(c.0).unwrap_or_default();
            *hourly_clicks.entry((dt.format("%Y-%m-%d").to_string(), dt.hour())).or_insert(0) += 1;
        }
        let batch: Vec<_> = buf_clicks.drain(..).collect();
        let _ = db.record_mouse_clicks_batch(&batch);
        for ((d, h), c) in hourly_clicks {
            let _ = db.upsert_hourly_mouse_clicks(&d, h, c);
        }
    }

    *pending_keys = 0;
    *pending_moves = 0;
    *pending_clicks = 0;
}
