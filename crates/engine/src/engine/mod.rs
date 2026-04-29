mod api;
mod completed_sessions;
mod error;
mod event_log;
mod export;
mod groups;
mod plan;
mod resolve;
mod runtime;
mod session_lifecycle;
mod session_registry;
mod stats;
mod traffic_sampler;
mod view;

pub use completed_sessions::CompletedSessionRecord;
pub use error::EngineError;
pub use export::{
    ActiveSessionExport, AddressExport, CompletedSessionExport, EngineConfigExport,
    EngineRuntimeExport, EngineStatusExport, InboundExport, ModeExport, OutboundExport,
    OutboundGroupExport, SessionAuthExport,
};
pub use groups::{UrlTestGroupState, UrlTestMemberState};
pub use plan::{
    EnginePlan, FallbackGroupPlan, OutboundTarget, SelectorGroupPlan, TargetId, TargetKind,
    TargetNode, UrlTestGroupPlan,
};
pub use resolve::{ResolvedLeafOutbound, ResolvedOutbound};
pub use runtime::Engine;
pub use runtime::RouteDecision;
pub use session_lifecycle::SessionHandle;
pub use session_registry::ActiveSession;
pub use stats::{EngineStatsSnapshot, SessionOutcome, UdpUpstreamStatsSnapshot};
