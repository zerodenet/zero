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
    /// Node push connector — actively reports to an external management
    /// endpoint.  Generic: the receiver can be a panel, monitoring system,
    /// or any HTTP service.
    #[serde(default)]
    pub push: PushConfig,
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
    #[serde(default)]
    pub log: LogConfig,
}

impl Default for RuntimeOptionsConfig {
    fn default() -> Self {
        Self {
            udp_upstream_idle_timeout_seconds: default_udp_upstream_idle_timeout_seconds(),
            log: LogConfig::default(),
        }
    }
}

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
    /// Flow hooks executed in registration order.
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
    /// Node push for heartbeat and command polling.
    #[serde(default)]
    pub push: PushConfig,
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
#[serde(tag = "type")]
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
        reality: Option<Box<InboundRealityConfig>>,
        #[serde(default)]
        ws: Option<WebSocketConfig>,
        #[serde(default)]
        grpc: Option<GrpcConfig>,
        #[serde(default)]
        h2: Option<H2Config>,
        #[serde(default)]
        http_upgrade: Option<HttpUpgradeConfig>,
        #[serde(default)]
        fallback: Option<FallbackConfig>,
        #[serde(default)]
        quic: Option<QuicConfig>,
        #[serde(default)]
        split_http: Option<SplitHttpConfig>,
    },
    #[serde(rename = "hysteria2")]
    Hysteria2 {
        password: String,
        #[serde(default)]
        cert_path: Option<String>,
        #[serde(default)]
        key_path: Option<String>,
    },
    #[serde(rename = "shadowsocks")]
    Shadowsocks {
        password: String,
        #[serde(default = "default_ss_cipher")]
        cipher: String,
    },
    #[serde(rename = "trojan")]
    Trojan {
        password: String,
        #[serde(default)]
        sni: Option<String>,
        #[serde(default)]
        tls: Option<TlsConfig>,
    },
}

impl InboundProtocolConfig {
    pub fn tls_config(&self) -> Option<&TlsConfig> {
        match self {
            Self::Vless { tls, .. } => tls.as_ref(),
            Self::Trojan { tls, .. } => tls.as_ref(),
            _ => None,
        }
    }
}

fn default_ss_cipher() -> String {
    "chacha20-ietf-poly1305".to_string()
}

impl InboundProtocolConfig {
    pub fn socks5_users(&self) -> &[Socks5UserConfig] {
        match self {
            Self::Socks5 { users } => users,
            Self::Mixed { socks5_users } => socks5_users,
            Self::HttpConnect | Self::Vless { .. } | Self::Hysteria2 { .. } | Self::Shadowsocks { .. } | Self::Trojan { .. } => &[],
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

    pub fn vless_reality(&self) -> Option<&InboundRealityConfig> {
        match self {
            Self::Vless { reality, .. } => reality.as_deref(),
            _ => None,
        }
    }

    pub fn vless_ws(&self) -> Option<&WebSocketConfig> {
        match self {
            Self::Vless { ws, .. } => ws.as_ref(),
            _ => None,
        }
    }

    pub fn vless_grpc(&self) -> Option<&GrpcConfig> {
        match self {
            Self::Vless { grpc, .. } => grpc.as_ref(),
            _ => None,
        }
    }

    pub fn vless_h2(&self) -> Option<&H2Config> {
        match self {
            Self::Vless { h2, .. } => h2.as_ref(),
            _ => None,
        }
    }

    pub fn vless_http_upgrade(&self) -> Option<&HttpUpgradeConfig> {
        match self {
            Self::Vless { http_upgrade, .. } => http_upgrade.as_ref(),
            _ => None,
        }
    }

    pub fn vless_split_http(&self) -> Option<&SplitHttpConfig> {
        match self {
            Self::Vless { split_http, .. } => split_http.as_ref(),
            _ => None,
        }
    }

    pub fn vless_fallback(&self) -> Option<&FallbackConfig> {
        match self {
            Self::Vless { fallback, .. } => fallback.as_ref(),
            _ => None,
        }
    }

    pub fn vless_quic(&self) -> Option<&QuicConfig> {
        match self {
            Self::Vless { quic, .. } => quic.as_ref(),
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
    pub flow: Option<String>,
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
pub struct InboundRealityConfig {
    pub private_key: String,
    #[serde(default)]
    pub short_ids: Vec<String>,
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default)]
    pub cipher_suites: Vec<String>,
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
pub struct GrpcConfig {
    #[serde(
        alias = "service_name",
        default = "default_grpc_service_names",
        deserialize_with = "deserialize_service_names"
    )]
    pub service_names: Vec<String>,
}

fn default_grpc_service_names() -> Vec<String> {
    vec!["/v2ray.core.proxy.vless.encap.GrpcService/Tun".to_string()]
}

fn deserialize_service_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct ServiceNames;

    impl<'de> Visitor<'de> for ServiceNames {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut names = Vec::new();
            while let Some(name) = seq.next_element::<String>()? {
                names.push(name);
            }
            if names.is_empty() {
                return Err(de::Error::invalid_length(0, &self));
            }
            Ok(names)
        }
    }

    deserializer.deserialize_any(ServiceNames)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct H2Config {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_h2_path")]
    pub path: String,
}

fn default_h2_path() -> String {
    "/".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpUpgradeConfig {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_http_upgrade_path")]
    pub path: String,
}

fn default_http_upgrade_path() -> String {
    "/".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitHttpConfig {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default = "default_split_http_path")]
    pub path: String,
}

fn default_split_http_path() -> String {
    "/".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackConfig {
    pub server: String,
    pub port: u16,
    #[serde(default)]
    pub alpn: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuicConfig {
    // Inbound
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
    // Outbound
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default)]
    pub insecure: bool,
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
        flow: Option<String>,
        #[serde(default)]
        mux_concurrency: Option<u32>,
        #[serde(default)]
        mux_idle_timeout_secs: Option<u64>,
        #[serde(default)]
        tls: Option<ClientTlsConfig>,
        #[serde(default)]
        reality: Option<Box<RealityConfig>>,
        #[serde(default)]
        ws: Option<WebSocketConfig>,
        #[serde(default)]
        grpc: Option<GrpcConfig>,
        #[serde(default)]
        h2: Option<H2Config>,
        #[serde(default)]
        http_upgrade: Option<HttpUpgradeConfig>,
        #[serde(default)]
        split_http: Option<SplitHttpConfig>,
        #[serde(default)]
        quic: Option<QuicConfig>,
    },
    #[serde(rename = "hysteria2")]
    Hysteria2 {
        server: String,
        port: u16,
        password: String,
        #[serde(default)]
        insecure: bool,
    },
    #[serde(rename = "shadowsocks")]
    Shadowsocks {
        server: String,
        port: u16,
        password: String,
        #[serde(default = "default_ss_cipher")]
        cipher: String,
    },
    #[serde(rename = "trojan")]
    Trojan {
        server: String,
        port: u16,
        password: String,
        #[serde(default)]
        sni: Option<String>,
        #[serde(default)]
        insecure: bool,
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
            OutboundGroupKind::Relay { proxies } => proxies.first().map(String::as_str),
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
    #[serde(rename = "relay")]
    Relay { proxies: Vec<String> },
}

impl OutboundGroupKind {
    pub fn members(&self) -> &[String] {
        match self {
            Self::Selector { outbounds, .. }
            | Self::Fallback { outbounds }
            | Self::UrlTest { outbounds, .. } => outbounds,
            Self::Relay { proxies } => proxies,
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
    /// Path to a GeoLite2-Country.mmdb file for the `geoip` condition.
    #[serde(default)]
    pub geoip_database: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRuleSetConfig {
    pub tag: String,
    #[serde(rename = "type")]
    pub source_type: RuleSetSourceType,
    /// Path to a local file, or fallback cache path for URL sources.
    pub path: String,
    /// URL to fetch the rule set from (required when type = "url").
    #[serde(default)]
    pub url: Option<String>,
    /// Re-fetch interval in seconds (default 86400 = 24h).
    #[serde(default = "default_rule_set_update_interval")]
    pub update_interval_seconds: u64,
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
    #[serde(rename = "url")]
    Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleSetFormatConfig {
    #[serde(rename = "domain-list")]
    DomainList,
    #[serde(rename = "cidr-list")]
    CidrList,
}

fn default_rule_set_update_interval() -> u64 {
    86400
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
    #[serde(rename = "domain-keyword")]
    DomainKeyword { values: Vec<String> },
    #[serde(rename = "ip")]
    Ip { values: Vec<IpNet> },
    #[serde(rename = "rule-set")]
    RuleSet { tag: String },
    #[serde(rename = "geoip")]
    GeoIp { values: Vec<String> },
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
