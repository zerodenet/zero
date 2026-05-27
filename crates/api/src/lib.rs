pub mod auth;
pub mod capabilities;
pub mod command;
pub mod error;
pub mod event;
pub mod flow;
pub mod query;
pub mod sink;
pub mod traits;
#[cfg(feature = "webhook")]
pub mod webhook;

pub use auth::{AuthContext, Permission};
pub use capabilities::{AdapterCapability, ApiCapabilities, SinkCapability};
pub use command::{
    CommandRequest, CommandResponse, ConfigApplyCommand, ConfigValidateCommand,
    DiagnosticsDnsLookupCommand, DiagnosticsProbeTargetCommand, DiagnosticsTraceRouteCommand,
    FlowCloseCommand, ModeSetCommand, PolicyProbeCommand, PolicySelectCommand,
    TunStartCommand, TunStopCommand,
};
pub use error::{ApiError, ApiErrorCode};
pub use event::{event_type, ApiEvent, EventFilter, PublishResult};
pub use flow::{
    AuthInfo, EndpointRef, FlowEventPayload, FlowOutcome, FlowTiming, Network, PolicyDecision,
    PolicyProbeCompletedPayload, PolicyProbeMember, PolicySelectedPayload, RouteDecision,
    TargetAddress, TrafficStats, WarningPayload,
};
pub use query::{
    CapabilitiesQuery, ConfigQuery, DiagnosticsQuery, FlowFilter, FlowGetQuery, FlowListQuery,
    HealthQuery, HealthSnapshot, PoliciesQuery, PolicyGetQuery, QueryRequest, QueryResponse,
    RuntimeQuery, SinkStatusSnapshot, SinksQuery, Snapshot, StatsQuery, TunStatusQuery,
    TunStatusSnapshot,
};
pub use sink::{
    CallbackEventSink, DeadLetterSink, JsonLineEventSink, MemorySink, RotatingFileSink,
    SinkManager, SinkStatus,
};
pub use traits::{ApiAuth, ApiCodec, CommandService, EventSink, EventSource, QueryService};
#[cfg(feature = "webhook")]
pub use webhook::{WebhookEventSink, WebhookEventSinkConfig};

pub const API_VERSION: &str = "zero.api.v1";
pub const EVENT_SCHEMA_VERSION: &str = "zero.event.v1";

pub type ApiResult<T> = Result<T, ApiError>;
pub type RawApiEvent = ApiEvent<serde_json::Value>;
