use std::sync::OnceLock;

use serde_json::json;
use zero_api::{
    AdapterCapability, ApiCapabilities, ApiError, ApiErrorCode, CommandRequest, CommandResponse,
    CommandService, ConfigApplyCommand, ConfigValidateCommand, EventFilter, EventSource,
    FlowFilter, FlowSnapshot, Network as ApiNetwork, Permission, QueryRequest, QueryResponse,
    QueryService, RawApiEvent, SinkCapability, SinkStatusSnapshot,
};
use zero_config::{ModeConfig, RuntimeConfig};

use super::error::EngineError;
use super::export::{completed_to_flow, session_to_flow};
use super::runtime::Engine;

// ── Build features registry (injected from the top-level binary) ───

static BUILD_FEATURES: OnceLock<Vec<String>> = OnceLock::new();

/// Register compiled cargo features. Called once at startup from the
/// top-level binary crate that controls feature gating.
pub fn register_build_features(features: Vec<String>) {
    let _ = BUILD_FEATURES.set(features);
}

fn build_features() -> Vec<String> {
    BUILD_FEATURES.get().cloned().unwrap_or_default()
}

impl QueryService for Engine {
    fn query(&self, request: QueryRequest) -> zero_api::ApiResult<QueryResponse> {
        query_engine(self, request)
    }
}

impl CommandService for Engine {
    fn execute(&self, command: CommandRequest) -> zero_api::ApiResult<CommandResponse> {
        execute_engine_command(self, command)
    }
}

impl EventSource for Engine {
    type Stream = Vec<RawApiEvent>;

    fn subscribe(&self, filter: EventFilter) -> zero_api::ApiResult<Self::Stream> {
        Ok(self.events_snapshot(&filter))
    }

    fn latest(&self, limit: usize, filter: EventFilter) -> zero_api::ApiResult<Vec<RawApiEvent>> {
        let mut events = self.events_snapshot(&filter);
        events.truncate(limit);
        Ok(events)
    }
}

fn query_engine(engine: &Engine, request: QueryRequest) -> zero_api::ApiResult<QueryResponse> {
    match request {
        QueryRequest::Capabilities(_) => Ok(QueryResponse::Capabilities(capabilities())),
        QueryRequest::Health(_) => Ok(QueryResponse::Health(zero_api::HealthSnapshot {
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            started_at_unix_ms: Some(engine.started_at_unix_ms()),
            healthy: true,
        })),
        QueryRequest::Config(_) => Ok(QueryResponse::Config(engine.export_config())),
        QueryRequest::Runtime(_) => Ok(QueryResponse::Runtime(engine.export_runtime())),
        QueryRequest::Stats(_) => Ok(QueryResponse::Stats(engine.stats_snapshot())),
        QueryRequest::ActiveFlows(query) => {
            let flows = engine
                .active_sessions()
                .iter()
                .map(session_to_flow)
                .filter(|flow| matches_flow_filter(flow, &query.filter))
                .take(query.limit.unwrap_or(usize::MAX))
                .collect();
            Ok(QueryResponse::ActiveFlows(flows))
        }
        QueryRequest::RecentFlows(query) => {
            let flows = engine
                .completed_sessions()
                .iter()
                .map(completed_to_flow)
                .filter(|flow| matches_flow_filter(flow, &query.filter))
                .take(query.limit.unwrap_or(usize::MAX))
                .collect();
            Ok(QueryResponse::RecentFlows(flows))
        }
        QueryRequest::Flow(query) => {
            let active = engine.active_sessions();
            if let Some(flow) = active
                .iter()
                .map(session_to_flow)
                .find(|flow| flow.id.to_string() == query.flow_id)
            {
                return Ok(QueryResponse::Flow(flow));
            }

            let completed = engine.completed_sessions();
            if let Some(record) = completed
                .iter()
                .find(|s| s.id.to_string() == query.flow_id)
            {
                // Flow variant holds FlowSnapshot; for completed flows we
                // convert the common fields. The consumer can check the
                // `outcome` field on the diagnostics endpoint for completion.
                let flow = session_to_flow_from_completed(record);
                return Ok(QueryResponse::Flow(flow));
            }

            Err(ApiError::new(
                ApiErrorCode::NotFound,
                format!("flow `{}` was not found", query.flow_id),
            ))
        }
        QueryRequest::Policies(_) => {
            let config = engine.export_config();
            Ok(QueryResponse::Policies(config.outbound_groups))
        }
        QueryRequest::Policy(query) => {
            let config = engine.export_config();
            let policy = config
                .outbound_groups
                .into_iter()
                .find(|policy| policy.tag == query.policy_tag)
                .ok_or_else(|| {
                    ApiError::new(
                        ApiErrorCode::NotFound,
                        format!("policy `{}` was not found", query.policy_tag),
                    )
                })?;
            Ok(QueryResponse::Policy(policy))
        }
        QueryRequest::Diagnostics(_) => {
            let stats = engine.stats_snapshot();
            Ok(QueryResponse::Diagnostics(json!({
                "healthy": true,
                "active_sessions": stats.active_sessions,
                "completed_sessions": stats.completed_sessions,
                "failed_sessions": stats.failed_sessions,
                "udp_upstream": stats.udp_upstream,
            })))
        }
        QueryRequest::Sinks(_) => Ok(QueryResponse::Sinks(SinkStatusSnapshot::default())),
        QueryRequest::TunStatus(_) => Ok(QueryResponse::TunStatus(
            zero_api::TunStatusSnapshot::default(),
        )),
    }
}

/// Convert a completed session to a FlowSnapshot for the `Flow` query variant.
/// The completed-specific fields (outcome, duration) are lost here; consumers
/// should use the RecentFlows query for full completed-session details.
fn session_to_flow_from_completed(record: &super::completed_sessions::CompletedSessionRecord) -> FlowSnapshot {
    use super::export::address_to_snapshot;
    use super::export::auth_to_snapshot;
    FlowSnapshot {
        id: record.id,
        inbound_tag: record.inbound_tag.clone(),
        outbound_tag: record.outbound_tag.clone(),
        target: address_to_snapshot(&record.target),
        port: record.port,
        protocol: protocol_name(record.protocol).to_owned(),
        auth: record.auth.as_ref().map(auth_to_snapshot),
        network: network_name(record.network).to_owned(),
        mode: record.mode.clone(),
        started_at_unix_ms: record.started_at_unix_ms,
        last_activity_at_unix_ms: record.last_activity_at_unix_ms,
        bytes_up: record.bytes_up,
        bytes_down: record.bytes_down,
        inbound_rx_bytes: record.inbound_rx_bytes,
        inbound_tx_bytes: record.inbound_tx_bytes,
        outbound_rx_bytes: record.outbound_rx_bytes,
        outbound_tx_bytes: record.outbound_tx_bytes,
        throughput_up_bps: 0,
        throughput_down_bps: 0,
        process_id: record.process_id,
        process_name: record.process_name.clone(),
    }
}

// ── Command dispatch ────────────────────────────────────────────────

fn execute_engine_command(
    engine: &Engine,
    command: CommandRequest,
) -> zero_api::ApiResult<CommandResponse> {
    match command {
        CommandRequest::ConfigValidate(command) => validate_config_command(command),
        CommandRequest::ConfigApply(command) => apply_config_command(engine, command),
        CommandRequest::PolicySelect(command) => {
            engine
                .set_selector_target(&command.policy_tag, &command.target_tag)
                .map_err(engine_error_to_api)?;
            Ok(CommandResponse {
                accepted: true,
                result: Some(json!({
                    "policy_tag": command.policy_tag,
                    "selected": command.target_tag,
                })),
            })
        }
        CommandRequest::FlowClose(command) => match engine.close_flow(&command.flow_id) {
            Ok(()) => Ok(CommandResponse {
                accepted: true,
                result: Some(json!({
                    "flow_id": command.flow_id,
                    "closed": true,
                })),
            }),
            Err(error) => Err(engine_error_to_api(error)),
        },
        CommandRequest::PolicyProbe(command) => {
            match engine.trigger_urltest_probe(&command.policy_tag) {
                Ok(()) => Ok(CommandResponse {
                    accepted: true,
                    result: Some(json!({
                        "policy_tag": command.policy_tag,
                        "probe_triggered": true,
                    })),
                }),
                Err(error) => Err(engine_error_to_api(error)),
            }
        }
        CommandRequest::DiagnosticsProbeTarget(cmd) => match engine.probe_target(&cmd.target_tag) {
            Ok(result) => Ok(CommandResponse {
                accepted: true,
                result: Some(result),
            }),
            Err(error) => Err(engine_error_to_api(error)),
        },
        CommandRequest::DiagnosticsDnsLookup(cmd) => match engine.dns_lookup(&cmd.hostname) {
            Ok(result) => Ok(CommandResponse {
                accepted: true,
                result: Some(result),
            }),
            Err(error) => Err(engine_error_to_api(error)),
        },
        CommandRequest::DiagnosticsTraceRoute(cmd) => {
            let protocol = cmd.protocol.as_deref().unwrap_or("tcp");
            match engine.trace_route(&cmd.target, cmd.port, protocol) {
                Ok(result) => Ok(CommandResponse {
                    accepted: true,
                    result: Some(result),
                }),
                Err(error) => Err(engine_error_to_api(error)),
            }
        }
        CommandRequest::ModeSet(cmd) => {
            let mode = match cmd.mode.as_str() {
                "rule" => ModeConfig::Rule,
                "direct" => ModeConfig::Direct,
                "global" => {
                    let outbound = cmd.outbound.ok_or_else(|| {
                        ApiError::new(
                            ApiErrorCode::InvalidArgument,
                            "mode `global` requires `outbound` field",
                        )
                    })?;
                    ModeConfig::Global { outbound }
                }
                other => {
                    return Err(ApiError::new(
                        ApiErrorCode::InvalidArgument,
                        format!("unknown mode `{other}`; expected `rule`, `direct`, or `global`"),
                    ));
                }
            };
            engine.set_mode(mode);
            Ok(CommandResponse::accepted())
        }
        CommandRequest::TunStart(_) | CommandRequest::TunStop(_) => Err(ApiError::new(
            ApiErrorCode::Internal,
            "TUN commands are handled by the proxy runtime, not the engine",
        )),
    }
}

fn apply_config_command(
    engine: &Engine,
    command: ConfigApplyCommand,
) -> zero_api::ApiResult<CommandResponse> {
    let raw = serde_json::to_string(&command.config).map_err(to_internal_error)?;
    let new_config = RuntimeConfig::parse(&raw).map_err(config_error_to_api)?;
    engine
        .reload_config(new_config)
        .map_err(engine_error_to_api)?;

    Ok(CommandResponse {
        accepted: true,
        result: Some(json!({ "applied": true })),
    })
}

fn validate_config_command(command: ConfigValidateCommand) -> zero_api::ApiResult<CommandResponse> {
    let raw = serde_json::to_string(&command.config).map_err(to_internal_error)?;
    let _config = RuntimeConfig::parse(&raw).map_err(config_error_to_api)?;

    Ok(CommandResponse {
        accepted: true,
        result: Some(json!({ "valid": true })),
    })
}

fn capabilities() -> ApiCapabilities {
    let mut capabilities = ApiCapabilities::new();
    capabilities.adapters = vec![AdapterCapability {
        kind: "in-process".to_owned(),
        enabled: true,
    }];
    capabilities.sinks = vec![SinkCapability {
        kind: "none".to_owned(),
        enabled: false,
    }];
    capabilities.features = vec![
        "query".to_owned(),
        "config-snapshot".to_owned(),
        "runtime-snapshot".to_owned(),
        "flow-snapshot".to_owned(),
        "policy-snapshot".to_owned(),
    ];
    capabilities.build_features = build_features();
    capabilities.permissions = vec![Permission::Read];
    capabilities
}

// ── Flow filter ─────────────────────────────────────────────────────

trait FlowFilterView {
    fn inbound_tag(&self) -> Option<&str>;
    fn network(&self) -> &str;
    fn principal_key(&self) -> Option<&str>;
}

impl FlowFilterView for FlowSnapshot {
    fn inbound_tag(&self) -> Option<&str> {
        self.inbound_tag.as_deref()
    }

    fn network(&self) -> &str {
        &self.network
    }

    fn principal_key(&self) -> Option<&str> {
        self.auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref())
    }
}

impl FlowFilterView for zero_api::CompletedFlowSnapshot {
    fn inbound_tag(&self) -> Option<&str> {
        self.inbound_tag.as_deref()
    }

    fn network(&self) -> &str {
        &self.network
    }

    fn principal_key(&self) -> Option<&str> {
        self.auth
            .as_ref()
            .and_then(|auth| auth.principal_key.as_deref())
    }
}

fn matches_flow_filter(flow: &impl FlowFilterView, filter: &FlowFilter) -> bool {
    if let Some(expected_inbound) = &filter.inbound_tag {
        if flow.inbound_tag() != Some(expected_inbound.as_str()) {
            return false;
        }
    }

    if let Some(expected_principal) = &filter.principal_key {
        if flow.principal_key() != Some(expected_principal.as_str()) {
            return false;
        }
    }

    if let Some(expected_network) = filter.network {
        if flow.network() != api_network_name(expected_network) {
            return false;
        }
    }

    true
}

fn api_network_name(network: ApiNetwork) -> &'static str {
    match network {
        ApiNetwork::Tcp => "tcp",
        ApiNetwork::Udp => "udp",
    }
}

// ── Name helpers (shared with export) ────────────────────────────────

fn protocol_name(protocol: zero_core::ProtocolType) -> &'static str {
    match protocol {
        zero_core::ProtocolType::Socks5 => "socks5",
        zero_core::ProtocolType::HttpConnect => "http-connect",
        zero_core::ProtocolType::Vless => "vless",
        zero_core::ProtocolType::Hysteria2 => "hysteria2",
        zero_core::ProtocolType::Shadowsocks => "shadowsocks",
        zero_core::ProtocolType::Trojan => "trojan",
        zero_core::ProtocolType::Vmess => "vmess",
        zero_core::ProtocolType::Mieru => "mieru",
        zero_core::ProtocolType::Unknown => "unknown",
    }
}

fn network_name(network: zero_core::Network) -> &'static str {
    match network {
        zero_core::Network::Tcp => "tcp",
        zero_core::Network::Udp => "udp",
    }
}

// ── Error conversions ───────────────────────────────────────────────

fn to_internal_error(error: serde_json::Error) -> ApiError {
    ApiError {
        code: ApiErrorCode::Internal,
        message: "failed to serialize engine query snapshot".to_owned(),
        field_path: None,
        cause: Some(error.to_string()),
    }
}

fn config_error_to_api(error: zero_config::ConfigError) -> ApiError {
    ApiError {
        code: ApiErrorCode::InvalidArgument,
        message: "config validation failed".to_owned(),
        field_path: None,
        cause: Some(error.to_string()),
    }
}

fn engine_error_to_api(error: EngineError) -> ApiError {
    match error {
        EngineError::CompiledFeatureDisabled { .. } => ApiError {
            code: ApiErrorCode::FeatureDisabled,
            message: "requested feature is not enabled in this build".to_owned(),
            field_path: None,
            cause: Some(error.to_string()),
        },
        EngineError::Config(error) => config_error_to_api(error),
        EngineError::SelectorGroupNotFound { .. } => ApiError {
            code: ApiErrorCode::NotFound,
            message: "policy was not found".to_owned(),
            field_path: Some("policy_tag".to_owned()),
            cause: Some(error.to_string()),
        },
        EngineError::SelectorGroupTypeMismatch { .. } => ApiError {
            code: ApiErrorCode::InvalidArgument,
            message: "target policy is not selectable".to_owned(),
            field_path: Some("policy_tag".to_owned()),
            cause: Some(error.to_string()),
        },
        EngineError::SelectorTargetNotFound { .. } | EngineError::MissingRouteTarget { .. } => {
            ApiError {
                code: ApiErrorCode::InvalidArgument,
                message: "policy target is invalid".to_owned(),
                field_path: Some("target_tag".to_owned()),
                cause: Some(error.to_string()),
            }
        }
        EngineError::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::NotFound => ApiError {
            code: ApiErrorCode::NotFound,
            message: io_err.to_string(),
            field_path: Some("flow_id".to_owned()),
            cause: Some(error.to_string()),
        },
        EngineError::Io(_) => ApiError {
            code: ApiErrorCode::InvalidArgument,
            message: error.to_string(),
            field_path: None,
            cause: None,
        },
        error => ApiError {
            code: ApiErrorCode::Internal,
            message: "command execution failed".to_owned(),
            field_path: None,
            cause: Some(error.to_string()),
        },
    }
}
