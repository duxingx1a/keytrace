use std::path::PathBuf;

fn app_data_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|p| PathBuf::from(p).join("KeyTrace"))
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub db_path: PathBuf,
    pub idle_timeout_ms: u64,
    pub retention_days: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 50555,
            db_path: app_data_dir().join("keytrace.db"),
            idle_timeout_ms: 10000,
            retention_days: 7,
        }
    }
}

impl Config {
    /// 始终使用默认配置，不再加载 config.json
    pub fn load() -> Self {
        Self::default()
    }
}
