use crate::db::Database;
use std::sync::Arc;
use std::time::Instant;

/// 活跃会话管理器
pub struct SessionManager {
    db: Arc<Database>,
    idle_timeout_ms: u64,

    /// 当前会话状态
    session_start_ms: Option<i64>,
    last_key_ms: Option<i64>,
    session_key_count: i64,
    in_session: bool,

    /// 最后按键时间（用于空闲检测）
    last_activity: Instant,
}

impl SessionManager {
    pub fn new(db: Arc<Database>, idle_timeout_ms: u64) -> Self {
        Self {
            db,
            idle_timeout_ms,
            session_start_ms: None,
            last_key_ms: None,
            session_key_count: 0,
            in_session: false,
            last_activity: Instant::now(),
        }
    }

    /// 按键事件回调
    pub fn on_key(&mut self, now_ms: i64) {
        let now = Instant::now();

        if !self.in_session {
            // 上一会话已结束，开始新会话
            self.session_start_ms = Some(now_ms);
            self.last_key_ms = Some(now_ms);
            self.session_key_count = 1;
            self.in_session = true;
        } else {
            // 检查是否超时
            let elapsed = now.duration_since(self.last_activity).as_millis() as u64;
            if elapsed >= self.idle_timeout_ms {
                // 超时了，结束当前会话，开始新会话
                if let (Some(start), Some(end)) = (self.session_start_ms, self.last_key_ms) {
                    // 只保存超过 1 个键的会话（避免单个点击污染）
                    if self.session_key_count > 1 {
                        let date = Self::ms_to_date(start);
                        let _ = self.db.save_session(&date, start, end, self.session_key_count);
                    }
                }
                self.session_start_ms = Some(now_ms);
                self.last_key_ms = Some(now_ms);
                self.session_key_count = 1;
                // in_session 保持 true
            } else {
                // 正常延续
                self.last_key_ms = Some(now_ms);
                self.session_key_count += 1;
            }
        }

        self.last_activity = now;
    }

    /// 空闲检测：关闭超时会话
    pub fn check_idle(&mut self) {
        if !self.in_session {
            return;
        }
        let elapsed = self.last_activity.elapsed().as_millis() as u64;
        if elapsed >= self.idle_timeout_ms {
            self.close_session();
        }
    }

    /// 强制关闭当前会话
    fn close_session(&mut self) {
        if let (Some(start), Some(end)) = (self.session_start_ms, self.last_key_ms) {
            if self.session_key_count > 1 {
                let date = Self::ms_to_date(start);
                let _ = self.db.save_session(&date, start, end, self.session_key_count);
            }
        }
        self.session_start_ms = None;
        self.last_key_ms = None;
        self.session_key_count = 0;
        self.in_session = false;
    }

    fn ms_to_date(ms: i64) -> String {
        let secs = ms / 1000;
        let naive = chrono::DateTime::from_timestamp(secs, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        naive
    }
}
