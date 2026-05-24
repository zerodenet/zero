use serde::Serialize;
use serde_json::json;
use zero_api::{
    AdapterCapability, ApiCapabilities, ApiError, ApiErrorCode, CommandRequest, CommandResponse,
    CommandService, ConfigApplyCommand, ConfigValidateCommand, EventFilter, EventSource,
    FlowFilter, FlowGetQuery, FlowListQuery, Network as ApiNetwork, Permission, PoliciesQuery,
    PolicyGetQuery, QueryRequest, QueryResponse, QueryService, RawApiEvent, SinkCapability,
    SinkStatusSnapshot, Snapshot,
};
use zero_config::{ModeConfig, RuntimeConfig};

use super::completed_sessions::CompletedSessionRecord;
use super::error::EngineError;
use super::export::{
    ActiveSessionExport, CompletedSessionExport, EngineConfigExport, EngineRuntimeExport,
    OutboundGroupExport,
};
use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::EngineStatsSnapshot;

impl QueryService for Engine {
    fn query(&self, request: QueryRequest) -> zero_api::ApiResult<QueryResponse> {
        query_engine(
            EngineQueryView {
                config: self.export_config(),
                runtime: self.export_runtime(),
                stats: self.stats_snapshot(),
                active_sessions: self.active_sessions(),
                completed_sessions: self.completed_sessions(),
            },
            request,
        )
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

struct EngineQueryView {
    config: EngineConfigExport,
    runtime: EngineRuntimeExport,
    stats: EngineStatsSnapshot,
    active_sessions: Vec<ActiveSession>,
    completed_sessions: Vec<CompletedSessionRecord>,
}

fn query_engine(
    view: EngineQueryView,
    request: QueryRequest,
) -> zero_api::ApiResult<QueryResponse> {
    match request {
        QueryRequest::Capabilities(_) => Ok(QueryResponse::Capabilities(capabilities())),
        QueryRequest::Health(_) => Ok(QueryResponse::Health(zero_api::HealthSnapshot {
            engine_version: env!("CARGO_PKG_VERSION").to_owned(),
            started_at_unix_ms: None,
            healthy: true,
        })),
        QueryRequest::Config(_) => snapshot_response(QueryResponse::Config, view.config),
        QueryRequest::Runtime(_) => snapshot_response(QueryResponse::Runtime, view.runtime),
        QueryRequest::Stats(_) => snapshot_response(QueryResponse::Stats, view.stats),
        QueryRequest::ActiveFlows(query) => snapshot_response(
            QueryResponse::Flows,
            active_flows(view.active_sessions, &query)?,
        ),
        QueryRequest::RecentFlows(query) => snapshot_response(
            QueryResponse::Flows,
            recent_flows(view.completed_sessions, &query)?,
        ),
        QueryRequest::Flow(query) => {
            query_flow(view.active_sessions, view.completed_sessions, query)
        }
        QueryRequest::Policies(PoliciesQuery) => {
            snapshot_response(QueryResponse::Policies, view.config.outbound_groups)
        }
        QueryRequest::Policy(query) => query_policy(view.config.outbound_groups, query),
        QueryRequest::Diagnostics(_) => snapshot_response(
            QueryResponse::Diagnostics,
            json!({
                "healthy": true,
                "active_sessions": view.stats.active_sessions,
                "completed_sessions": view.stats.completed_sessions,
                "failed_sessions": view.stats.failed_sessions,
                "udp_upstream": view.stats.udp_upstream,
            }),
        ),
        QueryRequest::Sinks(_) => Ok(QueryResponse::Sinks(SinkStatusSnapshot::default())),
    }
}

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
    capabilities.permissions = vec![Permission::Read];
    capabilities
}

fn snapshot_response(
    wrap: impl FnOnce(Snapshot) -> QueryResponse,
    value: impl Serialize,
) -> zero_api::ApiResult<QueryResponse> {
    Ok(wrap(Snapshot {
        value: serde_json::to_value(value).map_err(to_internal_error)?,
    }))
}

fn active_flows(
    sessions: Vec<ActiveSession>,
    query: &FlowListQuery,
) -> zero_api::ApiResult<Vec<ActiveSessionExport>> {
    let flows = sessions
        .iter()
        .map(ActiveSessionExport::from)
        .filter(|flow| matches_flow_filter(flow, &query.filter))
        .take(query.limit.unwrap_or(usize::MAX))
        .collect();
    Ok(flows)
}

fn recent_flows(
    sessions: Vec<CompletedSessionRecord>,
    query: &FlowListQuery,
) -> zero_api::ApiResult<Vec<CompletedSessionExport>> {
    let flows = sessions
        .iter()
        .map(CompletedSessionExport::from)
        .filter(|flow| matches_flow_filter(flow, &query.filter))
        .take(query.limit.unwrap_or(usize::MAX))
        .collect();
    Ok(flows)
}

fn query_flow(
    active_sessions: Vec<ActiveSession>,
    completed_sessions: Vec<CompletedSessionRecord>,
    query: FlowGetQuery,
) -> zero_api::ApiResult<QueryResponse> {
    if let Some(flow) = active_sessions
        .iter()
        .map(ActiveSessionExport::from)
        .find(|flow| flow.id.to_string() == query.flow_id)
    {
        return snapshot_response(QueryResponse::Flow, flow);
    }

    if let Some(flow) = completed_sessions
        .iter()
        .map(CompletedSessionExport::from)
        .find(|flow| flow.id.to_string() == query.flow_id)
    {
        return snapshot_response(QueryResponse::Flow, flow);
    }

    Err(ApiError::new(
        ApiErrorCode::NotFound,
        format!("flow `{}` was not found", query.flow_id),
    ))
}

fn query_policy(
    policies: Vec<OutboundGroupExport>,
    query: PolicyGetQuery,
) -> zero_api::ApiResult<QueryResponse> {
    let policy = policies
        .into_iter()
        .find(|policy| policy.tag == query.policy_tag)
        .ok_or_else(|| {
            ApiError::new(
                ApiErrorCode::NotFound,
                format!("policy `{}` was not found", query.policy_tag),
            )
        })?;

    snapshot_response(QueryResponse::Policy, policy)
}

trait FlowFilterView {
    fn inbound_tag(&self) -> Option<&str>;
    fn network(&self) -> &str;
    fn principal_key(&self) -> Option<&str>;
}

impl FlowFilterView for ActiveSessionExport {
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

impl FlowFilterView for CompletedSessionExport {
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
