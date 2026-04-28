use serde::{Deserialize, Serialize};

use crate::{ApiCapabilities, Network, Permission};

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
}

impl QueryRequest {
    pub fn required_permission(&self) -> Permission {
        Permission::Read
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryResponse {
    Capabilities(ApiCapabilities),
    Health(HealthSnapshot),
    Config(Snapshot),
    Runtime(Snapshot),
    Stats(Snapshot),
    Flows(Snapshot),
    Flow(Snapshot),
    Policies(Snapshot),
    Policy(Snapshot),
    Diagnostics(Snapshot),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGetQuery {
    pub flow_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyGetQuery {
    pub policy_tag: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub engine_version: String,
    pub started_at_unix_ms: Option<u64>,
    pub healthy: bool,
}
