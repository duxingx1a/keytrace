use crate::config::Config;
use crate::db::Database;
use crate::static_files::{self, StaticFiles};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tiny_http::{Header, Response, Server};

/// 实际绑定的端口（可能因端口占用而不同于配置值）
static ACTUAL_PORT: AtomicU16 = AtomicU16::new(0);

/// 获取 API 服务器实际绑定的端口
pub fn get_actual_port() -> u16 {
    ACTUAL_PORT.load(Ordering::Relaxed)
}

/// 启动 HTTP API 服务器
pub fn start_api(
    db: Arc<Database>,
    config: &Config,
    running: Arc<AtomicBool>,
    start_time: Instant,
) {
    let mut port = config.port;
    let server = loop {
        let addr = format!("127.0.0.1:{}", port);
        match Server::http(&addr) {
            Ok(s) => break s,
            Err(e) => {
                if port < config.port + 10 {
                    eprintln!("[keytrace] 端口 {} 被占用，尝试 {}...", port, port + 1);
                    port += 1;
                } else {
                    eprintln!("[keytrace] HTTP server 启动失败（已尝试 {} 个端口）: {}", 11, e);
                    return;
                }
            }
        }
    };
    ACTUAL_PORT.store(port, Ordering::Relaxed);

    println!("[keytrace] API server: http://127.0.0.1:{}", port);

    while running.load(Ordering::Relaxed) {
        match server.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(Some(request)) => {
                handle_request(request, &db, start_time);
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

fn handle_request(request: tiny_http::Request, db: &Database, start_time: Instant) {
    let url = request.url().to_string();
    let method = request.method().as_str().to_string();

    if method != "GET" {
        respond_json(request, 405, &serde_json::json!({"error": "method not allowed"}));
        return;
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let now_ms = chrono::Local::now().timestamp_millis();

    // 路由解析
    let response = if url == "/api/info" {
        let uptime = start_time.elapsed().as_secs();
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_secs": uptime,
            "uptime_str": format_uptime(uptime),
        })
    } else if url == "/api/stats/today" {
        let key_count = db.today_key_count(&today).unwrap_or(0);
        let (active_ms, session_keys) = db.today_active_stats(&today).unwrap_or((0, 0));
        let wpm = if active_ms > 0 {
            (session_keys as f64) / (active_ms as f64 / 60000.0)
        } else {
            0.0
        };
        serde_json::json!({
            "date": today,
            "total_keys": key_count,
            "active_ms": active_ms,
            "active_secs": active_ms / 1000,
            "session_keys": session_keys,
            "wpm": (wpm * 10.0).round() / 10.0,
        })
    } else if url.starts_with("/api/stats/range") {
        // /api/stats/range?from=2026-07-28&to=2026-07-28
        let from_date = extract_param(&url, "from").unwrap_or(today.as_str());
        let to_date = extract_param(&url, "to").unwrap_or(today.as_str());
        let total_keys = db.range_key_count(from_date, to_date).unwrap_or(0);
        let (active_ms, session_keys) = db.range_active_stats(from_date, to_date).unwrap_or((0, 0));
        let wpm = if active_ms > 0 {
            (session_keys as f64) / (active_ms as f64 / 60000.0)
        } else {
            0.0
        };
        // 日期转毫秒时间戳
        let from_ms = parse_ms(from_date, "0");
        let to_ms = parse_ms_end(to_date);
        let mouse_moves = db.range_mouse_move_count(from_ms, to_ms).unwrap_or(0);
        let mouse_clicks = db.range_mouse_click_count(from_ms, to_ms).unwrap_or(0);
        serde_json::json!({
            "from": from_date,
            "to": to_date,
            "total_keys": total_keys,
            "active_ms": active_ms,
            "active_secs": active_ms / 1000,
            "wpm": (wpm * 10.0).round() / 10.0,
            "mouse_moves": mouse_moves,
            "mouse_clicks": mouse_clicks,
        })
    } else if url == "/api/info/screens" {
        let screens = crate::screens::enumerate_screens();
        serde_json::json!({"screens": screens})
    } else if url.starts_with("/api/stats/hourly") {
        // /api/stats/hourly?date=2026-07-28
        let date = extract_param(&url, "date").unwrap_or(today.as_str());
        let hours_raw = db.hourly_key_counts(date).unwrap_or_default();
        let hours: Vec<serde_json::Value> = hours_raw
            .into_iter()
            .map(|(hour, count)| serde_json::json!({"hour": hour, "count": count}))
            .collect();
        serde_json::json!({"date": date, "hours": hours})
    } else if url.starts_with("/api/stats/daily") {
        // /api/stats/daily?days=7
        let days_str = extract_param(&url, "days").unwrap_or("7");
        let days: u32 = days_str.parse().unwrap_or(7);
        let days_raw = db.daily_key_counts(days).unwrap_or_default();
        let days_list: Vec<serde_json::Value> = days_raw
            .into_iter()
            .map(|(date, count)| serde_json::json!({"date": date, "count": count}))
            .collect();
        serde_json::json!({"days": days_list})
    } else if url.starts_with("/api/stats/trend") {
        // /api/stats/trend?from=&to=&metric=keys|wpm|mouse_moves|mouse_clicks
        let from_date = extract_param(&url, "from").unwrap_or(today.as_str());
        let to_date = extract_param(&url, "to").unwrap_or(today.as_str());
        let metric = extract_param(&url, "metric").unwrap_or("keys");
        // 1天模式强制hourly；否则取请求参数跨度（用于7天）或数据库最早日期（用于所有）
        let req_days = days_between(from_date, to_date).max(1);
        let actual_from = db.earliest_date().unwrap_or(None).unwrap_or(from_date.to_string());
        let effective_days = if req_days <= 7 { req_days } else { days_between(&actual_from, to_date).max(1) };
        let (granularity, out_type) = if req_days <= 1 { ("hourly", "hourly") }
            else if effective_days <= 90 { ("daily", "daily") }
            else if effective_days <= 730 { ("monthly", "monthly") }
            else { ("yearly", "yearly") };
        let from_ms = parse_ms(from_date, "0");
        let to_ms = parse_ms_end(to_date);

        let points: Vec<serde_json::Value> = match metric {
            "wpm" => {
                let raw = match granularity {
                    "hourly" => {
                        let r = db.hourly_wpm(from_date).unwrap_or_default();
                        r.iter().map(|(h, v)| serde_json::json!({"date": h.to_string(), "value": (v * 10.0).round() / 10.0})).collect()
                    }
                    "monthly" => {
                        let r = db.monthly_wpm(from_date, to_date).unwrap_or_default();
                        r.iter().map(|(d, v)| serde_json::json!({"date": d, "value": (v * 10.0).round() / 10.0})).collect()
                    }
                    "yearly" => {
                        let r = db.yearly_wpm(from_date, to_date).unwrap_or_default();
                        r.iter().map(|(d, v)| serde_json::json!({"date": d, "value": (v * 10.0).round() / 10.0})).collect()
                    }
                    _ => {
                        let r = db.daily_wpm(from_date, to_date).unwrap_or_default();
                        r.iter().map(|(d, v)| serde_json::json!({"date": d, "value": (v * 10.0).round() / 10.0})).collect()
                    }
                };
                raw
            }
            "mouse_moves" => {
                let raw = db.mouse_moves_trend(from_ms, to_ms, granularity).unwrap_or_default();
                raw.iter().map(|(d, c)| serde_json::json!({"date": d, "value": c})).collect()
            }
            "mouse_clicks" => {
                let raw = db.mouse_clicks_trend(from_ms, to_ms, granularity).unwrap_or_default();
                raw.iter().map(|(d, c)| serde_json::json!({"date": d, "value": c})).collect()
            }
            _ => {
                if granularity == "hourly" {
                    let raw = db.hourly_key_counts(from_date).unwrap_or_default();
                    raw.iter().map(|(h, c)| serde_json::json!({"date": h.to_string(), "value": c})).collect()
                } else {
                    let raw: Vec<(String, i64)> = match granularity {
                        "monthly" => db.monthly_key_counts(from_date, to_date).unwrap_or_default(),
                        "yearly" => db.yearly_key_counts(from_date, to_date).unwrap_or_default(),
                        _ => db.range_daily_key_counts(from_date, to_date).unwrap_or_default(),
                    };
                    raw.iter().map(|(d,c)| serde_json::json!({"date": d, "value": c})).collect()
                }
            }
        };
        serde_json::json!({"type": out_type, "points": points})
    } else if url.starts_with("/api/stats/keys") {
        // /api/stats/keys?from=...&to=... 或 /api/stats/keys?date=...
        let from = extract_param(&url, "from");
        let to = extract_param(&url, "to");
        if let (Some(from_date), Some(to_date)) = (from, to) {
            let keys = db.key_stats_by_range(from_date, to_date).unwrap_or_default();
            let key_list: Vec<serde_json::Value> = keys
                .into_iter()
                .map(|(code, count)| serde_json::json!({"key_code": code, "count": count}))
                .collect();
            serde_json::json!({"keys": key_list})
        } else {
            let date = extract_param(&url, "date").unwrap_or(&today);
            let keys = db.key_stats_by_date(date).unwrap_or_default();
            let key_list: Vec<serde_json::Value> = keys
                .into_iter()
                .map(|(code, count)| serde_json::json!({"key_code": code, "count": count}))
                .collect();
            serde_json::json!({"date": date, "keys": key_list})
        }
    } else if url.starts_with("/api/mouse/moves") {
        // /api/mouse/moves?from=...&to=...&limit=5000
        let from_str = extract_param(&url, "from").unwrap_or("");
        let to_str = extract_param(&url, "to").unwrap_or("");
        let limit_str = extract_param(&url, "limit").unwrap_or("5000");
        let default_from = (now_ms - 86400000).to_string();
        let from_ms = parse_ms(from_str, &default_from);
        let to_ms = parse_ms(to_str, &now_ms.to_string());
        let limit: i64 = limit_str.parse().unwrap_or(5000);
        let moves = db.mouse_moves_in_range(from_ms, to_ms, limit).unwrap_or_default();
        let move_list: Vec<serde_json::Value> = moves
            .into_iter()
            .map(|(x, y, w, h, screen_index)| {
                serde_json::json!({
                    "x": x,
                    "y": y,
                    "display_width": w,
                    "display_height": h,
                    "screen_index": screen_index
                })
            })
            .collect();
        serde_json::json!({"moves": move_list, "count": move_list.len()})
    } else {
        // 尝试 serve 前端静态文件
        return serve_static(request, &url);
    };

    respond_json(request, 200, &response);
}

fn serve_static(request: tiny_http::Request, url: &str) {
    let file_path = if url == "/" || url.is_empty() {
        "index.html"
    } else {
        let trimmed = &url[1..];
        // 防止路径遍历
        if trimmed.contains("..") {
            respond_json(request, 403, &serde_json::json!({"error": "forbidden"}));
            return;
        }
        trimmed
    };

    // 优先从嵌入资源读取
    if let Some(file) = StaticFiles::get(file_path) {
        let ct = static_files::mime_type(file_path);
        let response = Response::from_data(file.data.to_vec())
            .with_header(Header::from_bytes(&b"Content-Type"[..], ct.as_bytes()).unwrap());
        let _ = request.respond(response);
        return;
    }

    // 嵌入资源没找到（仅在开发模式 dist 不完整时）
    // 回退到文件系统：exe_dir/../../frontend/dist 或 exe_dir/frontend/dist
    let exe_dir = std::path::PathBuf::from(
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(&std::path::PathBuf::from("."))
    );
    let candidates = [
        exe_dir.join("../../frontend/dist"),
        exe_dir.join("frontend/dist"),
        exe_dir.join("../frontend/dist"),
    ];
    for dist_dir in &candidates {
        let full_path = dist_dir.join(file_path);
        if full_path.exists() && full_path.is_file() {
            let ct = static_files::mime_type(file_path);
            let mut file = std::fs::File::open(&full_path).unwrap();
            let mut content = Vec::new();
            file.read_to_end(&mut content).ok();
            let response = Response::from_data(content)
                .with_header(Header::from_bytes(&b"Content-Type"[..], ct.as_bytes()).unwrap());
            let _ = request.respond(response);
            return;
        }
    }

    respond_json(request, 404, &serde_json::json!({"error": "not found"}));
}

fn respond_json(request: tiny_http::Request, status: u16, data: &serde_json::Value) {
    let body = serde_json::to_string(data).unwrap_or_default();
    let ct = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let response = Response::from_string(body)
        .with_header(ct)
        .with_status_code(status);
    let _ = request.respond(response);
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}时{}分{}秒", h, m, s)
    } else if m > 0 {
        format!("{}分{}秒", m, s)
    } else {
        format!("{}秒", s)
    }
}

/// 从 URL 查询参数中取值（简易版）
fn extract_param<'a>(url: &'a str, key: &str) -> Option<&'a str> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            return parts.next();
        }
    }
    None
}

fn parse_ms(val: &str, default: &str) -> i64 {
    val.parse::<i64>().unwrap_or_else(|_| {
        // 尝试解析 ISO datetime "2026-07-28T23:59:59"
        if val.len() >= 19 {
            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&val[..19], "%Y-%m-%dT%H:%M:%S") {
                return dt.and_utc().timestamp_millis();
            }
        }
        // 尝试解析日期 "2026-07-28"
        if val.len() >= 10 {
            if let Ok(dt) = chrono::NaiveDate::parse_from_str(&val[..10], "%Y-%m-%d") {
                let datetime = dt.and_hms_opt(0, 0, 0).unwrap();
                return datetime.and_utc().timestamp_millis();
            }
        }
        default.parse().unwrap_or(0)
    })
}

fn parse_ms_end(val: &str) -> i64 {
    if val.len() >= 10 {
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(&val[..10], "%Y-%m-%d") {
            // 次日 00:00:00 的毫秒时间戳
            let next = dt.succ_opt().unwrap_or(dt);
            let datetime = next.and_hms_opt(0, 0, 0).unwrap();
            return datetime.and_utc().timestamp_millis();
        }
    }
    0
}

fn days_between(from: &str, to: &str) -> i64 {
    let parse = |s: &str| chrono::NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d");
    match (parse(from), parse(to)) {
        (Ok(f), Ok(t)) => (t - f).num_days(),
        _ => 0,
    }
}
