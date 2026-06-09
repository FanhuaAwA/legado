use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureMode {
    Normal,
    Developer,
    Unrestricted,
}

impl Default for SecureMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone)]
pub struct ReaderCoreOptions {
    pub app_data_dir: PathBuf,
    pub request_timeout_secs: u64,
    pub user_agent: Option<String>,
    pub secure_mode: SecureMode,
}

impl ReaderCoreOptions {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Self {
        Self {
            app_data_dir: app_data_dir.into(),
            request_timeout_secs: 35,
            user_agent: None,
            secure_mode: SecureMode::Normal,
        }
    }
}
