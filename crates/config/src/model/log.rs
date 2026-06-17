use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    /// Default minimum log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,
    /// File output targets.  Omit / empty array = stderr only.
    #[serde(default)]
    pub files: Vec<LogFileConfig>,
    /// Per-second rate limit (optional).  0 = unlimited.
    #[serde(default)]
    pub rate_limit: Option<LogRateLimit>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            files: Vec::new(),
            rate_limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogFileConfig {
    pub path: String,
    /// Per-file minimum level override.  Defaults to `log.level`.
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default = "default_log_max_bytes")]
    pub max_bytes: u64,
    #[serde(default = "default_log_max_files")]
    pub max_files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRateLimit {
    pub max_per_second: u64,
}

fn default_log_level() -> String {
    "info".to_owned()
}
fn default_log_max_bytes() -> u64 {
    10 * 1024 * 1024
}
fn default_log_max_files() -> usize {
    5
}
