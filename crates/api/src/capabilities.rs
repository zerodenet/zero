use serde::{Deserialize, Serialize};

use crate::{Permission, API_VERSION, EVENT_SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCapabilities {
    #[serde(default)]
    pub api_version: String,
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub adapters: Vec<AdapterCapability>,
    #[serde(default)]
    pub sinks: Vec<SinkCapability>,
    #[serde(default)]
    pub features: Vec<String>,
    /// Compiled cargo feature flags visible at runtime.
    #[serde(default)]
    pub build_features: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
}

impl ApiCapabilities {
    pub fn new() -> Self {
        Self {
            api_version: API_VERSION.to_owned(),
            schema_version: EVENT_SCHEMA_VERSION.to_owned(),
            adapters: Vec::new(),
            sinks: Vec::new(),
            features: Vec::new(),
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
