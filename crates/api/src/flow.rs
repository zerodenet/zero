use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEventPayload {
    pub flow_id: String,
    pub network: Network,
    pub inbound: EndpointRef,
    pub auth: Option<AuthInfo>,
    pub target: TargetAddress,
    pub route: RouteDecision,
    pub policy: Option<PolicyDecision>,
    pub outbound: Option<EndpointRef>,
    pub traffic: TrafficStats,
    pub timing: FlowTiming,
    pub outcome: FlowOutcome,
    /// Why the flow ended (standard close reason). `None` = normal / unspecified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointRef {
    pub tag: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthInfo {
    pub scheme: String,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub attributes: BTreeMap<String, String>,
}

impl AuthInfo {
    pub fn new(scheme: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            credential_id: None,
            principal_key: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetAddress {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteDecision {
    pub mode: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub tag: String,
    pub kind: String,
    pub selected: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrafficStats {
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub inbound_rx_bytes: Option<u64>,
    pub inbound_tx_bytes: Option<u64>,
    pub outbound_rx_bytes: Option<u64>,
    pub outbound_tx_bytes: Option<u64>,
    pub packets_up: Option<u64>,
    pub packets_down: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowTiming {
    pub started_at_unix_ms: u64,
    pub ended_at_unix_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowOutcome {
    DirectRelayed,
    ChainedRelayed,
    Blocked,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySelectedPayload {
    pub policy_tag: String,
    pub policy_kind: String,
    pub selected: String,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProbeCompletedPayload {
    pub policy_tag: String,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub started_at_unix_ms: u64,
    #[serde(default)]
    pub completed_at_unix_ms: u64,
    #[serde(default)]
    pub duration_ms: u64,
    pub selected: Option<String>,
    pub members: Vec<PolicyProbeMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProbeMember {
    pub target_tag: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarningPayload {
    pub code: String,
    pub message: String,
}
