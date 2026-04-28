use serde::{Deserialize, Serialize};

use crate::{Permission, API_VERSION, EVENT_SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCapabilities {
    pub api_version: String,
    pub schema_version: String,
    pub adapters: Vec<AdapterCapability>,
    pub sinks: Vec<SinkCapability>,
    pub features: Vec<String>,
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
