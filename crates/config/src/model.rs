use std::fs;
use std::path::{Path, PathBuf};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

use crate::ConfigError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub inbounds: Vec<InboundConfig>,
    #[serde(default)]
    pub outbounds: Vec<OutboundConfig>,
    #[serde(default)]
    pub outbound_groups: Vec<OutboundGroupConfig>,
    #[serde(default)]
    pub runtime: RuntimeOptionsConfig,
    #[serde(default)]
    pub mode: ModeConfig,
    pub route: RouteConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(skip)]
    pub source_dir: Option<PathBuf>,
}

impl RuntimeConfig {
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        Self::parse_with_source_dir(raw, None)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
            path: path.display().to_string(),
            source,
        })?;

        Self::parse_with_source_dir(&raw, path.parent().map(Path::to_path_buf))
    }

    pub fn source_dir(&self) -> Option<&Path> {
        self.source_dir.as_deref()
    }

    fn parse_with_source_dir(raw: &str, source_dir: Option<PathBuf>) -> Result<Self, ConfigError> {
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
        let mut config = serde_json::from_str::<Self>(raw)?;
        config.source_dir = source_dir;
        config.validate()?;

        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOptionsConfig {
    #[serde(default = "default_udp_upstream_idle_timeout_seconds")]
    pub udp_upstream_idle_timeout_seconds: u64,
}

impl Default for RuntimeOptionsConfig {
    fn default() -> Self {
        Self {
            udp_upstream_idle_timeout_seconds: default_udp_upstream_idle_timeout_seconds(),
        }
    }
}

const fn default_udp_upstream_idle_timeout_seconds() -> u64 {
    30
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default)]
    pub event_sinks: Vec<EventSinkConfig>,
    #[serde(default)]
    pub control: ControlApiConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventSinkConfig {
    #[serde(rename = "jsonl", alias = "file")]
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
#[serde(deny_unknown_fields)]
pub struct InboundConfig {
    pub tag: String,
    pub listen: ListenConfig,
    pub protocol: InboundProtocolConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListenConfig {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InboundProtocolConfig {
    #[serde(rename = "socks5")]
    Socks5 {
        #[serde(default)]
        users: Vec<Socks5UserConfig>,
    },
    #[serde(rename = "http-connect", alias = "http")]
    HttpConnect,
    #[serde(rename = "mixed")]
    Mixed {
        #[serde(default, alias = "users")]
        socks5_users: Vec<Socks5UserConfig>,
    },
    #[serde(rename = "vless")]
    Vless {
        users: Vec<VlessUserConfig>,
        #[serde(default)]
        tls: Option<TlsConfig>,
        #[serde(default)]
        ws: Option<WebSocketConfig>,
    },
}

impl InboundProtocolConfig {
    pub fn socks5_users(&self) -> &[Socks5UserConfig] {
        match self {
            Self::Socks5 { users } => users,
            Self::Mixed { socks5_users } => socks5_users,
            Self::HttpConnect | Self::Vless { .. } => &[],
        }
    }

    pub fn vless_users(&self) -> &[VlessUserConfig] {
        match self {
            Self::Vless { users, .. } => users,
            _ => &[],
        }
    }

    pub fn vless_tls(&self) -> Option<&TlsConfig> {
        match self {
            Self::Vless { tls, .. } => tls.as_ref(),
            _ => None,
        }
    }

    pub fn vless_ws(&self) -> Option<&WebSocketConfig> {
        match self {
            Self::Vless { ws, .. } => ws.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Socks5UserConfig {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VlessUserConfig {
    pub id: String,
    #[serde(default)]
    pub credential_id: Option<String>,
    #[serde(default)]
    pub principal_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    #[serde(default)]
    pub alpn: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientTlsConfig {
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default)]
    pub disable_sni: bool,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub alpn: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityConfig {
    pub public_key: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default)]
    pub cipher_suites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSocketConfig {
    #[serde(default = "default_ws_path")]
    pub path: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_ws_path() -> String {
    "/".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboundConfig {
    pub tag: String,
    pub protocol: OutboundProtocolConfig,
}

impl OutboundConfig {
    pub fn tag(&self) -> &str {
        &self.tag
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutboundProtocolConfig {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "block")]
    Block,
    #[serde(rename = "socks5")]
    Socks5 {
        server: String,
        port: u16,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
    },
    #[serde(rename = "vless")]
    Vless {
        server: String,
        port: u16,
        id: String,
        #[serde(default)]
        tls: Option<ClientTlsConfig>,
        #[serde(default)]
        reality: Option<Box<RealityConfig>>,
        #[serde(default)]
        ws: Option<WebSocketConfig>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundGroupConfig {
    pub tag: String,
    #[serde(flatten)]
    pub group: OutboundGroupKind,
}

impl OutboundGroupConfig {
    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn active_outbound(&self) -> Option<&str> {
        match &self.group {
            OutboundGroupKind::Selector {
                outbounds,
                selected,
                default,
            } => selected
                .as_deref()
                .or(default.as_deref())
                .or_else(|| outbounds.first().map(String::as_str)),
            OutboundGroupKind::Fallback { outbounds } => outbounds.first().map(String::as_str),
            OutboundGroupKind::UrlTest { outbounds, .. } => outbounds.first().map(String::as_str),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutboundGroupKind {
    #[serde(rename = "selector")]
    Selector {
        outbounds: Vec<String>,
        #[serde(default)]
        default: Option<String>,
        #[serde(default)]
        selected: Option<String>,
    },
    #[serde(rename = "fallback")]
    Fallback { outbounds: Vec<String> },
    #[serde(rename = "urltest")]
    UrlTest {
        outbounds: Vec<String>,
        url: String,
        #[serde(default = "default_urltest_interval_seconds")]
        interval_seconds: u64,
    },
}

impl OutboundGroupKind {
    pub fn members(&self) -> &[String] {
        match self {
            Self::Selector { outbounds, .. }
            | Self::Fallback { outbounds }
            | Self::UrlTest { outbounds, .. } => outbounds,
        }
    }
}

const fn default_urltest_interval_seconds() -> u64 {
    300
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum ModeConfig {
    #[default]
    #[serde(rename = "rule")]
    Rule,
    #[serde(rename = "global")]
    Global { outbound: String },
    #[serde(rename = "direct")]
    Direct,
}

impl ModeConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Global { .. } => "global",
            Self::Direct => "direct",
        }
    }

    pub fn outbound(&self) -> Option<&str> {
        match self {
            Self::Global { outbound } => Some(outbound),
            Self::Rule | Self::Direct => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig {
    #[serde(default)]
    pub rule_sets: Vec<RouteRuleSetConfig>,
    #[serde(default)]
    pub rules: Vec<RouteRuleConfig>,
    #[serde(rename = "final")]
    pub final_action: RouteActionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRuleSetConfig {
    pub tag: String,
    #[serde(rename = "type")]
    pub source_type: RuleSetSourceType,
    pub path: String,
    pub format: RuleSetFormatConfig,
}

impl RouteRuleSetConfig {
    pub fn source_path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSetSourceType {
    #[serde(rename = "file")]
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSetFormatConfig {
    #[serde(rename = "domain-list")]
    DomainList,
    #[serde(rename = "cidr-list")]
    CidrList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRuleConfig {
    pub condition: RuleConditionConfig,
    pub action: RouteActionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleConditionConfig {
    #[serde(rename = "domain")]
    Domain { values: Vec<String> },
    #[serde(rename = "ip")]
    Ip { values: Vec<IpNet> },
    #[serde(rename = "rule-set")]
    RuleSet { tag: String },
    #[serde(rename = "and")]
    And { items: Vec<RuleConditionConfig> },
    #[serde(rename = "or")]
    Or { items: Vec<RuleConditionConfig> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RouteActionConfig {
    #[serde(rename = "direct")]
    Direct,
    #[serde(rename = "reject", alias = "block")]
    Reject,
    #[serde(rename = "route")]
    Route { outbound: String },
}
