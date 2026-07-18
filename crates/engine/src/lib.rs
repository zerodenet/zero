mod api;
mod completed_sessions;
mod error;
mod event_log;
mod export;
mod groups;
mod handle;
mod hook;
mod outbound_health;
mod passive_relay_health;
mod plan;
mod probe_trigger;
mod resolve;
mod runtime;
mod session_lifecycle;
mod session_registry;
mod stats;
mod traffic_sampler;
mod view;

pub use api::register_build_features;
pub use completed_sessions::CompletedSessionRecord;
pub use error::EngineError;
pub use event_log::EventsSinceResult;
// Re-export snapshot types from zero-api so downstream code doesn't need
// to import from two different crates for the same logical types.
pub use groups::{UrlTestGroupState, UrlTestMemberState};
pub use handle::{EngineHandle, EventSubscriber};
pub use hook::{BlockReason, FlowContext, FlowHook, FlowHookChain, FlowTraffic};
pub use passive_relay_health::{PassiveRelayHealthKey, PassiveRelayOutcome, PassiveRelaySelection};
pub use plan::{
    EnginePlan, FallbackGroupPlan, LoadBalanceGroupPlan, OutboundTarget, SelectorGroupPlan,
    TargetId, TargetKind, TargetNode, UrlTestGroupPlan,
};
pub use probe_trigger::{ProbeTrigger, ProbeTriggerRegistry};
pub use resolve::{OutboundIdentity, ResolvedLeafOutbound, ResolvedOutbound};
pub use runtime::Engine;
pub use runtime::RouteDecision;
pub use session_lifecycle::SessionHandle;
pub use session_registry::ActiveSession;
pub use stats::SessionOutcome;
pub use zero_api::{
    AddressSnapshot, AuthSnapshot, CompletedFlowSnapshot, ConfigSnapshot, FlowSnapshot,
    ListenerSnapshot, ModeSnapshot, OutboundTargetSnapshot, PolicyMemberSnapshot, PolicySnapshot,
    RuntimeSnapshot, StatsSnapshot, StatusSnapshot,
};
// Re-export stats sub-types from zero-api.
pub use zero_api::{OutboundTrafficStats, UdpUpstreamStats};
pub use zero_api::{PolicyProbeCompletedPayload, PolicyProbeMember};
