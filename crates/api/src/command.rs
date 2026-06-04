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
    #[serde(rename = "diagnostics.dns_lookup")]
    DiagnosticsDnsLookup(DiagnosticsDnsLookupCommand),
    #[serde(rename = "diagnostics.trace_route")]
    DiagnosticsTraceRoute(DiagnosticsTraceRouteCommand),
    #[serde(rename = "mode.set")]
    ModeSet(ModeSetCommand),
    #[serde(rename = "tun.start")]
    TunStart(TunStartCommand),
    #[serde(rename = "tun.stop")]
    TunStop(TunStopCommand),
}

impl CommandRequest {
    pub fn required_permission(&self) -> Permission {
        match self {
            Self::ConfigValidate(_) | Self::ConfigApply(_) => Permission::Config,
            Self::FlowClose(_) | Self::PolicySelect(_) | Self::PolicyProbe(_) => {
                Permission::Control
            }
            Self::DiagnosticsProbeTarget(_)
            | Self::DiagnosticsDnsLookup(_)
            | Self::DiagnosticsTraceRoute(_)
            | Self::ModeSet(_)
            | Self::TunStart(_)
            | Self::TunStop(_) => Permission::Admin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResponse {
    #[serde(default)]
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCloseCommand {
    #[serde(default)]
    pub flow_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySelectCommand {
    #[serde(default)]
    pub policy_tag: String,
    #[serde(default)]
    pub target_tag: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyProbeCommand {
    #[serde(default)]
    pub policy_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigApplyCommand {
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Default for ConfigApplyCommand {
    fn default() -> Self {
        Self {
            config: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsProbeTargetCommand {
    #[serde(default)]
    pub target_tag: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSetCommand {
    /// One of: "rule", "global", "direct"
    #[serde(default)]
    pub mode: String,
    /// Required when mode is "global" — the outbound tag to route all traffic to.
    #[serde(default)]
    pub outbound: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsDnsLookupCommand {
    #[serde(default)]
    pub hostname: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticsTraceRouteCommand {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStartCommand {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub addr: String,
    #[serde(default = "default_tun_mtu")]
    pub mtu: u16,
    #[serde(default = "default_tun_mask")]
    pub mask: String,
    #[serde(default)]
    pub tag: String,
}

impl Default for TunStartCommand {
    fn default() -> Self {
        Self {
            name: None,
            addr: String::new(),
            mtu: default_tun_mtu(),
            mask: default_tun_mask(),
            tag: String::new(),
        }
    }
}

fn default_tun_mtu() -> u16 {
    1500
}
fn default_tun_mask() -> String {
    "255.255.255.0".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStopCommand;
