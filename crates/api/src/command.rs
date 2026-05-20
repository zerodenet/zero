use serde::{Deserialize, Serialize};

use crate::Permission;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum CommandRequest {
    #[serde(rename = "config.validate")]
    ConfigValidate(ConfigValidateCommand),
    #[serde(rename = "config.apply")]
    ConfigApply(ConfigApplyCommand),
    #[serde(rename = "flows.close")]
    FlowClose(FlowCloseCommand),
    #[serde(rename = "policies.select")]
    PolicySelect(PolicySelectCommand),
    #[serde(rename = "policies.probe")]
    PolicyProbe(PolicyProbeCommand),
    #[serde(rename = "diagnostics.probe_target")]
    DiagnosticsProbeTarget(DiagnosticsProbeTargetCommand),
    #[serde(rename = "mode.set")]
    ModeSet(ModeSetCommand),
}

impl CommandRequest {
    pub fn required_permission(&self) -> Permission {
        match self {
            Self::ConfigValidate(_) | Self::ConfigApply(_) => Permission::Config,
            Self::FlowClose(_) | Self::PolicySelect(_) | Self::PolicyProbe(_) => {
                Permission::Control
            }
            Self::DiagnosticsProbeTarget(_) | Self::ModeSet(_) => Permission::Admin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResponse {
    pub accepted: bool,
    pub result: Option<serde_json::Value>,
}

impl CommandResponse {
    pub fn accepted() -> Self {
        Self {
            accepted: true,
            result: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigValidateCommand {
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCloseCommand {
    pub flow_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySelectCommand {
    pub policy_tag: String,
    pub target_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProbeCommand {
    pub policy_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigApplyCommand {
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsProbeTargetCommand {
    pub target_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSetCommand {
    /// One of: "rule", "global", "direct"
    pub mode: String,
    /// Required when mode is "global" — the outbound tag to route all traffic to.
    #[serde(default)]
    pub outbound: Option<String>,
}
