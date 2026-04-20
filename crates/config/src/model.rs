use std::fs;
use std::path::Path;

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
    pub route: RouteConfig,
}

impl RuntimeConfig {
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
        let config = serde_json::from_str::<Self>(raw)?;
        config.validate()?;

        Ok(config)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
            path: path.display().to_string(),
            source,
        })?;

        Self::parse(&raw)
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
    Socks5,
    #[serde(rename = "http-connect", alias = "http")]
    HttpConnect,
    #[serde(rename = "mixed")]
    Mixed,
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
    Socks5 { server: String, port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteConfig {
    #[serde(default)]
    pub rules: Vec<RouteRuleConfig>,
    #[serde(rename = "final")]
    pub final_action: RouteActionConfig,
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
