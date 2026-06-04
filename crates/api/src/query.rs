use serde::{Deserialize, Serialize};

use crate::{ApiCapabilities, Network, Permission, SinkStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryRequest {
    Capabilities(CapabilitiesQuery),
    Health(HealthQuery),
    Config(ConfigQuery),
    Runtime(RuntimeQuery),
    Stats(StatsQuery),
    ActiveFlows(FlowListQuery),
    RecentFlows(FlowListQuery),
    Flow(FlowGetQuery),
    Policies(PoliciesQuery),
    Policy(PolicyGetQuery),
    Diagnostics(DiagnosticsQuery),
    Sinks(SinksQuery),
    TunStatus(TunStatusQuery),
}

impl QueryRequest {
    pub fn required_permission(&self) -> Permission {
        Permission::Read
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryResponse {
    Capabilities(ApiCapabilities),
    Health(HealthSnapshot),
    Config(crate::ConfigSnapshot),
    Runtime(crate::RuntimeSnapshot),
    Stats(crate::StatsSnapshot),
    ActiveFlows(Vec<crate::FlowSnapshot>),
    RecentFlows(Vec<crate::CompletedFlowSnapshot>),
    Flow(crate::FlowSnapshot),
    Policies(Vec<crate::PolicySnapshot>),
    Policy(crate::PolicySnapshot),
    Diagnostics(serde_json::Value),
    Sinks(SinkStatusSnapshot),
    TunStatus(TunStatusSnapshot),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoliciesQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinksQuery;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowListQuery {
    pub limit: Option<usize>,
    pub filter: FlowFilter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFilter {
    pub principal_key: Option<String>,
    pub inbound_tag: Option<String>,
    pub network: Option<Network>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGetQuery {
    #[serde(default)]
    pub flow_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyGetQuery {
    #[serde(default)]
    pub policy_tag: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    #[serde(default)]
    pub engine_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u64>,
    #[serde(default)]
    pub healthy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinkStatusSnapshot {
    #[serde(default)]
    pub sinks: Vec<SinkStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStatusQuery;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStatusSnapshot {
    #[serde(default)]
    pub running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}
