use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default)]
    pub event_sinks: Vec<EventSinkConfig>,
    #[serde(default)]
    pub control: ControlApiConfig,
    /// Flow hooks executed in registration order.
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
    /// Path to dead-letter queue file for failed event deliveries.
    /// When set, events that exhaust retry attempts are persisted here
    /// as JSON lines instead of being silently dropped.
    #[serde(default)]
    pub dead_letter_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EventSinkConfig {
    #[serde(rename = "jsonl")]
    JsonLines {
        tag: String,
        path: String,
        #[serde(default)]
        events: Vec<String>,
        #[serde(default)]
        source_id: Option<String>,
    },
    #[serde(rename = "webhook")]
    Webhook {
        tag: String,
        url: String,
        #[serde(default)]
        events: Vec<String>,
        #[serde(default)]
        source_id: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
        #[serde(default)]
        api_key_env: Option<String>,
        #[serde(default)]
        allow_insecure: bool,
    },
}

impl EventSinkConfig {
    pub fn tag(&self) -> &str {
        match self {
            Self::JsonLines { tag, .. } | Self::Webhook { tag, .. } => tag,
        }
    }

    pub fn events(&self) -> &[String] {
        match self {
            Self::JsonLines { events, .. } | Self::Webhook { events, .. } => events,
        }
    }

    pub fn source_id(&self) -> Option<&str> {
        match self {
            Self::JsonLines { source_id, .. } | Self::Webhook { source_id, .. } => {
                source_id.as_deref()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ControlApiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub listen: Option<ListenConfig>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum HookConfig {
    #[serde(rename = "ipc")]
    Ipc {
        socket: String,
        #[serde(default = "default_hook_timeout_ms")]
        timeout_ms: u64,
    },
}

fn default_hook_timeout_ms() -> u64 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PushConfig {
    /// Receiver endpoint URL.  When set, the node pushes heartbeats here.
    #[serde(default)]
    pub url: Option<String>,
    /// Node identifier sent to the receiver.
    #[serde(default)]
    pub node_id: Option<String>,
    /// Authentication key for the receiver.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Environment variable name for the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Heartbeat interval in seconds (default 30).
    #[serde(default = "default_push_heartbeat_interval")]
    pub heartbeat_interval_seconds: u64,
    /// Whether to poll for pending commands from the receiver.
    #[serde(default)]
    pub pull_commands: bool,
    /// Command polling interval in seconds (default 10).
    #[serde(default = "default_push_command_poll_interval")]
    pub command_poll_interval_seconds: u64,
}

fn default_push_heartbeat_interval() -> u64 {
    30
}
fn default_push_command_poll_interval() -> u64 {
    10
}

impl PushConfig {
    pub fn enabled(&self) -> bool {
        self.url.is_some() && self.node_id.is_some()
    }
}
