use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::info;
use zero_api::{EventFilter, RawApiEvent};
use zero_config::{ModeConfig, RuntimeConfig};
use zero_core::{Address, Session};
use zero_router::{RouteAction, RuleSet};

use super::completed_sessions::{CompletedSessionHistory, CompletedSessionRecord};
use super::error::EngineError;
use super::event_log::EngineEventLog;
use super::groups::{OutboundGroupStateStore, UrlTestGroupState, UrlTestMemberState};
use super::plan::{EnginePlan, TargetId, TargetKind};
use super::resolve::{
    resolve_target_chains, resolve_target_id, ResolvedLeafOutbound, ResolvedOutbound,
};
use super::session_lifecycle::SessionHandle;
use super::session_registry::{ActiveSession, SessionRegistry};
use super::stats::{EngineStats, EngineStatsSnapshot, SessionOutcome};
use super::view::PlanView;

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) config: Arc<RuntimeConfig>,
    pub(crate) plan: Arc<EnginePlan>,
    pub(crate) router: Arc<RuleSet>,
    next_session_id: Arc<AtomicU64>,
    session_registry: Arc<SessionRegistry>,
    completed_sessions: Arc<CompletedSessionHistory>,
    event_log: Arc<EngineEventLog>,
    stats: Arc<EngineStats>,
    pub(crate) outbound_group_state: Arc<OutboundGroupStateStore>,
    udp_upstream_idle_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDecision<'a> {
    Route(&'a str),
    Direct,
    Reject,
}

impl<'a> RouteDecision<'a> {
    fn to_owned(self) -> RouteAction {
        match self {
            Self::Route(tag) => RouteAction::Route(tag.to_owned()),
            Self::Direct => RouteAction::Direct,
            Self::Reject => RouteAction::Reject,
        }
    }
}

impl<'a> From<&'a RouteAction> for RouteDecision<'a> {
    fn from(value: &'a RouteAction) -> Self {
        match value {
            RouteAction::Route(tag) => Self::Route(tag),
            RouteAction::Direct => Self::Direct,
            RouteAction::Reject => Self::Reject,
        }
    }
}

impl Engine {
    pub fn new(config: RuntimeConfig) -> Result<Self, EngineError> {
        let router = Arc::new(config.route.compile(config.source_dir())?);
        let plan = Arc::new(EnginePlan::build(&config)?);
        let udp_upstream_idle_timeout =
            Duration::from_secs(config.runtime.udp_upstream_idle_timeout_seconds);
        let outbound_group_state = OutboundGroupStateStore::shared();

        for &group_id in plan.selector_groups() {
            let group = plan
                .target(group_id)
                .expect("engine plan should resolve selector group");
            let TargetKind::Selector(selector) = group.kind() else {
                continue;
            };
            outbound_group_state.initialize_selector(group_id, selector.initial_member());
        }

        for &group_id in plan.urltest_groups() {
            let group = plan
                .target(group_id)
                .expect("engine plan should resolve urltest group");
            let TargetKind::UrlTest(urltest) = group.kind() else {
                continue;
            };
            if !urltest.members().is_empty() {
                outbound_group_state.initialize_urltest(
                    group_id,
                    urltest.initial_member(),
                    urltest.members(),
                );
            }
        }

        Ok(Self {
            config: Arc::new(config),
            plan,
            router,
            next_session_id: Arc::new(AtomicU64::new(1)),
            session_registry: SessionRegistry::shared(),
            completed_sessions: CompletedSessionHistory::shared(),
            event_log: EngineEventLog::shared(),
            stats: EngineStats::shared(),
            outbound_group_state,
            udp_upstream_idle_timeout,
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let config = RuntimeConfig::load_from_path(path)?;
        Self::new(config)
    }

    pub fn config(&self) -> &RuntimeConfig {
        self.config.as_ref()
    }

    pub fn plan(&self) -> &EnginePlan {
        self.plan.as_ref()
    }

    pub fn with_udp_upstream_idle_timeout(mut self, timeout: Duration) -> Self {
        self.udp_upstream_idle_timeout = timeout;
        self
    }

    pub fn udp_upstream_idle_timeout(&self) -> Duration {
        self.udp_upstream_idle_timeout
    }

    pub fn mode_kind(&self) -> &'static str {
        self.config.mode.kind()
    }

    pub fn route_for(&self, address: &Address) -> RouteAction {
        self.route_decision(address).to_owned()
    }

    pub fn route_decision<'a>(&'a self, address: &Address) -> RouteDecision<'a> {
        match &self.config.mode {
            ModeConfig::Rule => self.router.decide_ref(address).into(),
            ModeConfig::Direct => RouteDecision::Direct,
            ModeConfig::Global { outbound } => RouteDecision::Route(outbound.as_str()),
        }
    }

    pub fn resolve_route_decision<'a>(
        &'a self,
        action: RouteDecision<'a>,
    ) -> Result<ResolvedOutbound<'a>, EngineError> {
        match action {
            RouteDecision::Direct => Ok(ResolvedOutbound::Single(ResolvedLeafOutbound::Direct {
                tag: None,
            })),
            RouteDecision::Reject => Ok(ResolvedOutbound::Single(ResolvedLeafOutbound::Block {
                tag: None,
            })),
            RouteDecision::Route(tag) => self.resolve_target(tag),
        }
    }

    pub fn resolve_route_action<'a>(
        &'a self,
        action: &'a RouteAction,
    ) -> Result<ResolvedOutbound<'a>, EngineError> {
        self.resolve_route_decision(action.into())
    }

    pub fn resolve_target_id<'a>(&'a self, target_id: TargetId) -> Option<ResolvedOutbound<'a>> {
        resolve_target_id(&self.plan, &self.outbound_group_state, target_id)
    }

    pub fn resolve_target_chains(&self, target_id: TargetId) -> Vec<Vec<TargetId>> {
        resolve_target_chains(&self.plan, &self.outbound_group_state, target_id)
    }

    pub fn target_tag(&self, target_id: TargetId) -> Option<&str> {
        self.plan.target(target_id).map(|target| target.tag())
    }

    fn resolve_target<'a>(&'a self, tag: &'a str) -> Result<ResolvedOutbound<'a>, EngineError> {
        let Some(target_id) = self.plan.target_id(tag) else {
            return Err(EngineError::MissingRouteTarget {
                tag: tag.to_owned(),
            });
        };

        self.resolve_target_id(target_id)
            .ok_or_else(|| EngineError::MissingRouteTarget {
                tag: tag.to_owned(),
            })
    }

    pub fn stats_snapshot(&self) -> EngineStatsSnapshot {
        self.stats.snapshot()
    }

    pub fn active_sessions(&self) -> Vec<ActiveSession> {
        self.session_registry.snapshot()
    }

    pub fn completed_sessions(&self) -> Vec<CompletedSessionRecord> {
        self.completed_sessions.snapshot()
    }

    pub fn events_snapshot(&self, filter: &EventFilter) -> Vec<RawApiEvent> {
        self.event_log.snapshot(filter)
    }

    pub fn set_selector_target(
        &self,
        group_tag: &str,
        target_tag: &str,
    ) -> Result<(), EngineError> {
        let group_id =
            self.plan
                .target_id(group_tag)
                .ok_or_else(|| EngineError::SelectorGroupNotFound {
                    tag: group_tag.to_owned(),
                })?;
        let group = self
            .plan
            .target(group_id)
            .expect("engine plan should resolve selector group");
        let TargetKind::Selector(selector) = group.kind() else {
            return Err(EngineError::SelectorGroupTypeMismatch {
                tag: group_tag.to_owned(),
            });
        };
        let target_id =
            self.plan
                .target_id(target_tag)
                .ok_or_else(|| EngineError::SelectorTargetNotFound {
                    group_tag: group_tag.to_owned(),
                    target_tag: target_tag.to_owned(),
                })?;
        if !selector.contains_member(target_id) {
            return Err(EngineError::SelectorTargetNotFound {
                group_tag: group_tag.to_owned(),
                target_tag: target_tag.to_owned(),
            });
        }
        let view = PlanView::new(&self.plan);

        let previous = self
            .outbound_group_state
            .selector_selected_target(group_id)
            .map(|target_id| view.target_tag_owned(target_id))
            .or_else(|| Some(view.target_tag_owned(selector.initial_member())));
        self.outbound_group_state
            .update_selector(group_id, target_id);
        info!(
            group_tag = group_tag,
            previous = previous.as_deref().unwrap_or("-"),
            selected = target_tag,
            "selector group target changed"
        );
        Ok(())
    }

    pub fn urltest_state(&self, group_id: TargetId) -> Option<UrlTestGroupState> {
        self.outbound_group_state.urltest_state(group_id)
    }

    pub fn urltest_selected_target(&self, group_id: TargetId) -> Option<TargetId> {
        self.outbound_group_state.urltest_selected_target(group_id)
    }

    pub fn update_urltest_state(
        &self,
        group_id: TargetId,
        selected: TargetId,
        latency_ms: Option<u64>,
        members: Vec<UrlTestMemberState>,
    ) {
        self.outbound_group_state
            .update_urltest(group_id, selected, latency_ms, members);
    }

    pub fn prepare_session(&self, session: &mut Session, inbound_tag: &str) {
        session.id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        session.inbound_tag = Some(inbound_tag.to_owned());
        self.session_registry
            .insert(session, self.config.mode.kind());
        self.stats.record_start();
    }

    pub fn set_session_outbound(&self, session: &Session) {
        self.session_registry
            .update_outbound_tag(session.id, session.outbound_tag.as_deref());
    }

    pub fn record_session_upload(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_upload(session_id, bytes);
    }

    pub fn record_session_download(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_download(session_id, bytes);
    }

    pub fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_inbound_rx(session_id, bytes);
    }

    pub fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_inbound_tx(session_id, bytes);
    }

    pub fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_outbound_rx(session_id, bytes);
    }

    pub fn record_session_outbound_tx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_outbound_tx(session_id, bytes);
    }

    pub fn record_udp_upstream_association_created(&self) {
        self.stats.record_udp_upstream_association_created();
    }

    pub fn record_udp_upstream_association_reused(&self) {
        self.stats.record_udp_upstream_association_reused();
    }

    pub fn record_udp_upstream_association_closed(&self) {
        self.stats.record_udp_upstream_association_closed();
    }

    pub fn record_udp_upstream_association_idle_timeout(&self) {
        self.stats.record_udp_upstream_association_idle_timeout();
    }

    pub fn record_udp_upstream_association_dropped(&self) {
        self.stats.record_udp_upstream_association_dropped();
    }

    pub fn record_udp_upstream_association_failed(&self) {
        self.stats.record_udp_upstream_association_failed();
    }

    pub fn record_udp_upstream_send_failure(&self) {
        self.stats.record_udp_upstream_send_failure();
    }

    pub fn record_udp_upstream_recv_failure(&self) {
        self.stats.record_udp_upstream_recv_failure();
    }

    pub fn record_udp_upstream_packet_sent(&self) {
        self.stats.record_udp_upstream_packet_sent();
    }

    pub fn record_udp_upstream_packet_received(&self) {
        self.stats.record_udp_upstream_packet_received();
    }

    pub fn finish_session(
        &self,
        session_id: u64,
        outcome: SessionOutcome,
    ) -> Option<CompletedSessionRecord> {
        let record = self.session_registry.finish(session_id, outcome)?;
        self.stats.record_finish(outcome);
        self.completed_sessions.push(record.clone());
        self.event_log
            .push_flow_completed(&record, |tag| self.outbound_protocol_for_tag(tag));
        Some(record)
    }

    pub fn track_session(&self, session_id: u64) -> SessionHandle {
        SessionHandle::new(self.clone(), session_id)
    }

    fn outbound_protocol_for_tag(&self, tag: &str) -> Option<&'static str> {
        if tag == "direct" {
            return Some("direct");
        }
        if tag == "block" {
            return Some("block");
        }

        self.config
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == tag)
            .map(|outbound| match outbound.protocol {
                zero_config::OutboundProtocolConfig::Direct => "direct",
                zero_config::OutboundProtocolConfig::Block => "block",
                zero_config::OutboundProtocolConfig::Socks5 { .. } => "socks5",
                zero_config::OutboundProtocolConfig::Vless { .. } => "vless",
            })
    }
}
