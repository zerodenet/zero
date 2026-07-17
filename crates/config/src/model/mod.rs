use std::fs;
use std::path::{Path, PathBuf};

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
        config.normalize();
        config.validate()?;

        Ok(config)
    }

    fn normalize(&mut self) {
        for inbound in &mut self.inbounds {
            inbound.protocol.normalize();
        }

        for outbound in &mut self.outbounds {
            outbound.protocol.normalize();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOptionsConfig {
    #[serde(default = "default_udp_upstream_idle_timeout_seconds")]
    pub udp_upstream_idle_timeout_seconds: u64,
    /// Global URL used by end-to-end outbound latency probes.
    #[serde(default)]
    pub latency_test_url: Option<String>,
    #[serde(default)]
    pub network: NetworkOptionsConfig,
    #[serde(default)]
    pub udp: UdpPolicyConfig,
    #[serde(default)]
    pub log: LogConfig,
    /// Optional DNS subsystem configuration. Omit for system resolver.
    #[serde(default)]
    pub dns: Option<DnsConfig>,
}

impl Default for RuntimeOptionsConfig {
    fn default() -> Self {
        Self {
            udp_upstream_idle_timeout_seconds: default_udp_upstream_idle_timeout_seconds(),
            latency_test_url: None,
            network: NetworkOptionsConfig::default(),
            udp: UdpPolicyConfig::default(),
            log: LogConfig::default(),
            dns: None,
        }
    }
}

pub const DEFAULT_LATENCY_TEST_URL: &str = "http://www.gstatic.com/generate_204";

impl RuntimeOptionsConfig {
    pub fn effective_latency_test_url(&self) -> &str {
        self.latency_test_url_or(None)
    }

    pub fn latency_test_url_or<'a>(&'a self, fallback: Option<&'a str>) -> &'a str {
        self.latency_test_url
            .as_deref()
            .or(fallback)
            .unwrap_or(DEFAULT_LATENCY_TEST_URL)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkOptionsConfig {
    /// MTU requested from the TUN backend and used by its user-space stack.
    #[serde(default = "default_network_mtu")]
    pub mtu: u16,
}

impl Default for NetworkOptionsConfig {
    fn default() -> Self {
        Self {
            mtu: default_network_mtu(),
        }
    }
}

const fn default_network_mtu() -> u16 {
    1500
}

const fn default_udp_upstream_idle_timeout_seconds() -> u64 {
    30
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UdpPolicyConfig {
    #[serde(default = "default_udp_enabled")]
    pub enabled: bool,
}

impl Default for UdpPolicyConfig {
    fn default() -> Self {
        Self {
            enabled: default_udp_enabled(),
        }
    }
}

const fn default_udp_enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", deny_unknown_fields)]
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

mod api;
mod dns;
mod inbound;
mod log;
mod outbound;
mod route;
mod transport;

pub use api::*;
pub use dns::*;
pub use inbound::*;
pub use log::*;
pub use outbound::*;
pub use route::*;
pub use transport::*;
