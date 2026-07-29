use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        // 确保父目录存在
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // 开启 WAL 模式：允许一写多读并发，避免 flush 时阻塞 API 查询
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS keystats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                hour INTEGER NOT NULL,
                key_code INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 1,
                UNIQUE(date, hour, key_code)
            );

            CREATE TABLE IF NOT EXISTS mouse_moves (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                display_width INTEGER NOT NULL,
                display_height INTEGER NOT NULL,
                screen_index INTEGER NOT NULL DEFAULT -1
            );

            CREATE TABLE IF NOT EXISTS mouse_clicks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                button TEXT NOT NULL,
                display_width INTEGER NOT NULL,
                display_height INTEGER NOT NULL,
                screen_index INTEGER NOT NULL DEFAULT -1
            );

            CREATE TABLE IF NOT EXISTS active_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                start_ms INTEGER NOT NULL,
                end_ms INTEGER NOT NULL,
                key_count INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS hourly_keys (
                date TEXT NOT NULL,
                hour INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(date, hour)
            );

            CREATE TABLE IF NOT EXISTS hourly_mouse_moves (
                date TEXT NOT NULL,
                hour INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(date, hour)
            );

            CREATE TABLE IF NOT EXISTS hourly_mouse_clicks (
                date TEXT NOT NULL,
                hour INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(date, hour)
            );

            -- 索引
            CREATE INDEX IF NOT EXISTS idx_keystats_date ON keystats(date);
            CREATE INDEX IF NOT EXISTS idx_mouse_moves_ts ON mouse_moves(timestamp);
            CREATE INDEX IF NOT EXISTS idx_mouse_clicks_ts ON mouse_clicks(timestamp);
            CREATE INDEX IF NOT EXISTS idx_sessions_date ON active_sessions(date);
            ",
        )?;
        Ok(())
    }

    // ---- 按键统计 ----

    /// 获取今日按键总数
    pub fn today_key_count(&self, date: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(count), 0) FROM keystats WHERE date = ?1",
            params![date],
            |row| row.get(0),
        )
    }

    /// 获取指定日期每小时按键数
    pub fn hourly_key_counts(&self, date: &str) -> Result<Vec<(u32, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT hour, SUM(count) FROM keystats WHERE date = ?1 GROUP BY hour ORDER BY hour"
        )?;
        let rows = stmt.query_map(params![date], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取最近 N 天每天按键数
    pub fn daily_key_counts(&self, days: u32) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date, SUM(count) FROM keystats GROUP BY date ORDER BY date DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![days], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    // ---- 小时预聚合 ━━━━━━━━━━━━━━━━━━━━━━━━━

    pub fn upsert_hourly_keys(&self, date: &str, hour: u32, count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO hourly_keys (date, hour, count) VALUES (?1, ?2, ?3)
             ON CONFLICT(date, hour) DO UPDATE SET count = count + ?3",
            params![date, hour, count],
        )?;
        Ok(())
    }

    pub fn upsert_hourly_mouse_moves(&self, date: &str, hour: u32, count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO hourly_mouse_moves (date, hour, count) VALUES (?1, ?2, ?3)
             ON CONFLICT(date, hour) DO UPDATE SET count = count + ?3",
            params![date, hour, count],
        )?;
        Ok(())
    }

    pub fn upsert_hourly_mouse_clicks(&self, date: &str, hour: u32, count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO hourly_mouse_clicks (date, hour, count) VALUES (?1, ?2, ?3)
             ON CONFLICT(date, hour) DO UPDATE SET count = count + ?3",
            params![date, hour, count],
        )?;
        Ok(())
    }

    // ---- 活跃会话 ----

    /// 保存一个活跃会话
    pub fn save_session(&self, date: &str, start_ms: i64, end_ms: i64, key_count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO active_sessions (date, start_ms, end_ms, key_count) VALUES (?1, ?2, ?3, ?4)",
            params![date, start_ms, end_ms, key_count],
        )?;
        Ok(())
    }

    /// 获取数据最早日期（用于趋势粒度判断）
    pub fn earliest_date(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT MIN(date) FROM keystats",
            [],
            |row| row.get(0),
        )
    }

    /// 获取日期范围内按天聚合的按键数
    pub fn range_daily_key_counts(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date, SUM(count) FROM keystats WHERE date >= ?1 AND date <= ?2 GROUP BY date ORDER BY date"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内按月聚合的按键数
    pub fn monthly_key_counts(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT substr(date, 1, 7), SUM(count) FROM keystats WHERE date >= ?1 AND date <= ?2 GROUP BY substr(date, 1, 7) ORDER BY 1"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内按年聚合的按键数
    pub fn yearly_key_counts(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT substr(date, 1, 4), SUM(count) FROM keystats WHERE date >= ?1 AND date <= ?2 GROUP BY substr(date, 1, 4) ORDER BY 1"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内的按键总数
    pub fn range_key_count(&self, from_date: &str, to_date: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(count), 0) FROM keystats WHERE date >= ?1 AND date <= ?2",
            params![from_date, to_date],
            |row| row.get(0),
        )
    }

    /// 获取日期范围内的活跃会话统计（总时长ms、总按键数）
    pub fn range_active_stats(&self, from_date: &str, to_date: &str) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(end_ms - start_ms), 0), COALESCE(SUM(key_count), 0)
             FROM active_sessions WHERE date >= ?1 AND date <= ?2",
            params![from_date, to_date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }

    /// 获取时间范围内的鼠标移动记录数
    pub fn range_mouse_move_count(&self, from_ms: i64, to_ms: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM mouse_moves WHERE timestamp >= ?1 AND timestamp < ?2",
            params![from_ms, to_ms],
            |row| row.get(0),
        )
    }

    /// 获取时间范围内的鼠标点击次数
    pub fn range_mouse_click_count(&self, from_ms: i64, to_ms: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM mouse_clicks WHERE timestamp >= ?1 AND timestamp < ?2",
            params![from_ms, to_ms],
            |row| row.get(0),
        )
    }

    /// 获取指定日期的活跃会话总时长(毫秒)和总按键数
    pub fn today_active_stats(&self, date: &str) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT COALESCE(SUM(end_ms - start_ms), 0), COALESCE(SUM(key_count), 0)
             FROM active_sessions WHERE date = ?1",
            params![date],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(result)
    }

    /// 获取指定日期每小时的手速 (WPM)
    pub fn hourly_wpm(&self, date: &str) -> Result<Vec<(u32, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT CAST(((start_ms / 3600000) + 8) % 24 AS INTEGER),
                    CAST(SUM(key_count) AS REAL) / NULLIF(SUM(end_ms - start_ms) / 60000.0, 0)
             FROM active_sessions WHERE date = ?1 GROUP BY 1 ORDER BY 1"
        )?;
        let rows = stmt.query_map(params![date], |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内按天聚合的手速 (WPM = 总按键 / (总活跃毫秒 / 60000))
    pub fn daily_wpm(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date, CAST(SUM(key_count) AS REAL) / NULLIF(SUM(end_ms - start_ms) / 60000.0, 0)
             FROM active_sessions WHERE date >= ?1 AND date <= ?2 GROUP BY date ORDER BY date"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内按月聚合的手速
    pub fn monthly_wpm(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT substr(date,1,7), CAST(SUM(key_count) AS REAL) / NULLIF(SUM(end_ms - start_ms) / 60000.0, 0)
             FROM active_sessions WHERE date >= ?1 AND date <= ?2 GROUP BY substr(date,1,7) ORDER BY 1"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 获取日期范围内按年聚合的手速
    pub fn yearly_wpm(&self, from_date: &str, to_date: &str) -> Result<Vec<(String, f64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT substr(date,1,4), CAST(SUM(key_count) AS REAL) / NULLIF(SUM(end_ms - start_ms) / 60000.0, 0)
             FROM active_sessions WHERE date >= ?1 AND date <= ?2 GROUP BY substr(date,1,4) ORDER BY 1"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| Ok((row.get(0)?, row.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 按小时/天/月/年统计鼠标移动次数（ts 为毫秒时间戳）
    pub fn mouse_moves_trend(&self, from_ms: i64, to_ms: i64, granularity: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let fmt = match granularity {
            "hourly" => "%H",
            "monthly" => "%Y-%m",
            "yearly" => "%Y",
            _ => "%Y-%m-%d",
        };
        let loc = if granularity == "hourly" { ", 'localtime'" } else { "" };
        let sql = format!(
            "SELECT strftime('{fmt}', timestamp/1000, 'unixepoch'{loc}), COUNT(*) FROM mouse_moves
             WHERE timestamp >= ?1 AND timestamp < ?2 GROUP BY 1 ORDER BY 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![from_ms, to_ms], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    /// 按小时/天/月/年统计鼠标点击次数
    pub fn mouse_clicks_trend(&self, from_ms: i64, to_ms: i64, granularity: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let fmt = match granularity {
            "hourly" => "%H",
            "monthly" => "%Y-%m",
            "yearly" => "%Y",
            _ => "%Y-%m-%d",
        };
        let loc = if granularity == "hourly" { ", 'localtime'" } else { "" };
        let sql = format!(
            "SELECT strftime('{fmt}', timestamp/1000, 'unixepoch'{loc}), COUNT(*) FROM mouse_clicks
             WHERE timestamp >= ?1 AND timestamp < ?2 GROUP BY 1 ORDER BY 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![from_ms, to_ms], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut result = Vec::new();
        for row in rows { result.push(row?); }
        Ok(result)
    }

    // ---- 批量写入（配合内存缓冲区） ----

    /// 批量 upsert 按键（合并后调用）
    pub fn record_key_batch(&self, date: &str, hour: u32, key_code: u32, count: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO keystats (date, hour, key_code, count) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(date, hour, key_code) DO UPDATE SET count = count + ?4",
            params![date, hour, key_code, count],
        )?;
        Ok(())
    }

    /// 批量写入鼠标移动
    pub fn record_mouse_moves_batch(&self, moves: &[(i64, i32, i32, i32, i32, i32)]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO mouse_moves (timestamp, x, y, display_width, display_height, screen_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (ts, x, y, w, h, screen_index) in moves {
                stmt.execute(params![ts, x, y, w, h, screen_index])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 批量写入鼠标点击
    pub fn record_mouse_clicks_batch(
        &self,
        clicks: &[(i64, i32, i32, crate::hooks::MouseButton, i32, i32, i32)],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO mouse_clicks (timestamp, x, y, button, display_width, display_height, screen_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (ts, x, y, button, w, h, screen_index) in clicks {
                stmt.execute(params![ts, x, y, button.as_str(), w, h, screen_index])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 获取指定日期的各键码统计
    pub fn key_stats_by_date(&self, date: &str) -> Result<Vec<(u32, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key_code, SUM(count) FROM keystats WHERE date = ?1 GROUP BY key_code ORDER BY SUM(count) DESC"
        )?;
        let rows = stmt.query_map(params![date], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// 获取日期范围内各键码统计
    pub fn key_stats_by_range(&self, from_date: &str, to_date: &str) -> Result<Vec<(u32, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT key_code, SUM(count) FROM keystats WHERE date >= ?1 AND date <= ?2 GROUP BY key_code ORDER BY SUM(count) DESC"
        )?;
        let rows = stmt.query_map(params![from_date, to_date], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// 获取指定时间范围的鼠标移动数据
    pub fn mouse_moves_in_range(&self, from_ms: i64, to_ms: i64, limit: i64) -> Result<Vec<(i32, i32, i32, i32, i32)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT x, y, display_width, display_height, screen_index FROM mouse_moves
             WHERE timestamp >= ?1 AND timestamp < ?2
             ORDER BY timestamp ASC
             LIMIT ?3"
        )?;
        let rows = stmt.query_map(params![from_ms, to_ms, limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    // ---- 数据清理 ----

    /// 删除 N 天前的原始数据（含预聚合表）
    pub fn cleanup_old_data(&self, retention_days: u32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let cutoff_date = chrono::Local::now()
            .date_naive()
            .checked_sub_signed(chrono::Duration::days(retention_days as i64))
            .unwrap();
        let cutoff_str = cutoff_date.format("%Y-%m-%d").to_string();
        // 日期转毫秒时间戳（UTC 午夜 —— mouse 表存的是 UTC epoch ms）
        let cutoff_ms = cutoff_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        conn.execute("DELETE FROM keystats WHERE date < ?1", params![&cutoff_str])?;
        conn.execute("DELETE FROM mouse_moves WHERE timestamp < ?1", params![cutoff_ms])?;
        conn.execute("DELETE FROM mouse_clicks WHERE timestamp < ?1", params![cutoff_ms])?;
        conn.execute("DELETE FROM active_sessions WHERE date < ?1", params![&cutoff_str])?;
        conn.execute("DELETE FROM hourly_keys WHERE date < ?1", params![&cutoff_str])?;
        conn.execute("DELETE FROM hourly_mouse_moves WHERE date < ?1", params![&cutoff_str])?;
        conn.execute("DELETE FROM hourly_mouse_clicks WHERE date < ?1", params![&cutoff_str])?;
        Ok(())
    }
}
