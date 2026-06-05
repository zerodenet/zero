use serde::{Deserialize, Serialize};

use crate::{Permission, API_ID, EVENT_SCHEMA_ID};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCapabilities {
    #[serde(default)]
    pub api_id: String,
    #[serde(default)]
    pub schema_id: String,
    #[serde(default)]
    pub adapters: Vec<AdapterCapability>,
    #[serde(default)]
    pub sinks: Vec<SinkCapability>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub protocols: Vec<ProtocolCapability>,
    /// Compiled cargo feature flags visible at runtime.
    #[serde(default)]
    pub build_features: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
}

impl ApiCapabilities {
    pub fn new() -> Self {
        Self {
            api_id: API_ID.to_owned(),
            schema_id: EVENT_SCHEMA_ID.to_owned(),
            adapters: Vec::new(),
            sinks: Vec::new(),
            features: Vec::new(),
            protocols: Vec::new(),
            build_features: Vec::new(),
            permissions: Vec::new(),
        }
    }
}

impl Default for ApiCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCapability {
    pub kind: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkCapability {
    pub kind: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCapability {
    pub protocol: String,
    pub feature: String,
    pub compiled: bool,
    pub status: String,
    pub compatibility_baseline: String,
    pub inbound: ProtocolNetworkCapability,
    pub outbound: ProtocolNetworkCapability,
    #[serde(default)]
    pub transports: Vec<String>,
    pub mux: CapabilityState,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolNetworkCapability {
    pub tcp: CapabilityState,
    pub udp: CapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityState {
    pub supported: bool,
    pub level: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl CapabilityState {
    pub fn supported() -> Self {
        Self {
            supported: true,
            level: "supported".to_owned(),
            notes: Vec::new(),
        }
    }

    pub fn partial(notes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            supported: true,
            level: "partial".to_owned(),
            notes: notes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn experimental(notes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            supported: true,
            level: "experimental".to_owned(),
            notes: notes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn unsupported(notes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            supported: false,
            level: "unsupported".to_owned(),
            notes: notes.into_iter().map(Into::into).collect(),
        }
    }

    pub fn not_applicable() -> Self {
        Self {
            supported: false,
            level: "not_applicable".to_owned(),
            notes: Vec::new(),
        }
    }
}
