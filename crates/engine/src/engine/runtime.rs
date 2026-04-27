use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_config::{InboundConfig, InboundProtocolConfig, ModeConfig, RuntimeConfig};
use zero_core::{Address, Session};
use zero_platform_tokio::{TokioListener, TokioResolver, TokioSocket};
use zero_router::{RouteAction, RuleSet};

use crate::ProtocolInventory;

use super::completed_sessions::{CompletedSessionHistory, CompletedSessionRecord};
use super::error::EngineError;
use super::logging::log_selector_group_target_changed;
use super::metered::StreamTraffic;
use super::outbound_group_state::OutboundGroupStateStore;
use super::plan::{EnginePlan, TargetKind};
use super::resolve::{resolve_target_id, ResolvedLeafOutbound, ResolvedOutbound};
use super::session_lifecycle::SessionHandle;
use super::session_registry::{ActiveSession, SessionRegistry};
use super::stats::{EngineStats, EngineStatsSnapshot, SessionOutcome};
use super::view::PlanView;

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) config: Arc<RuntimeConfig>,
    pub(crate) plan: Arc<EnginePlan>,
    pub(crate) router: Arc<RuleSet>,
    pub(crate) resolver: TokioResolver,
    pub(crate) protocols: ProtocolInventory,
    pub(crate) next_session_id: Arc<AtomicU64>,
    pub(crate) session_registry: Arc<SessionRegistry>,
    pub(crate) completed_sessions: Arc<CompletedSessionHistory>,
    pub(crate) stats: Arc<EngineStats>,
    pub(crate) outbound_group_state: Arc<OutboundGroupStateStore>,
    pub(crate) udp_upstream_idle_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteDecision<'a> {
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
        let protocols = ProtocolInventory::default();
        protocols.validate_config(&config)?;
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
            resolver: TokioResolver,
            protocols,
            next_session_id: Arc::new(AtomicU64::new(1)),
            session_registry: SessionRegistry::shared(),
            completed_sessions: CompletedSessionHistory::shared(),
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

    pub fn protocols(&self) -> &ProtocolInventory {
        &self.protocols
    }

    pub fn route_for(&self, address: &Address) -> RouteAction {
        self.route_decision(address).to_owned()
    }

    pub(crate) fn route_decision<'a>(&'a self, address: &Address) -> RouteDecision<'a> {
        match &self.config.mode {
            ModeConfig::Rule => self.router.decide_ref(address).into(),
            ModeConfig::Direct => RouteDecision::Direct,
            ModeConfig::Global { outbound } => RouteDecision::Route(outbound.as_str()),
        }
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
        log_selector_group_target_changed(group_tag, previous.as_deref(), target_tag);
        Ok(())
    }

    pub async fn run(&self) -> Result<(), EngineError> {
        self.run_until(async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => info!("shutdown signal received"),
                Err(error) => warn!(error = %error, "failed to listen for ctrl-c; stopping engine"),
            }
        })
        .await
    }

    pub async fn run_until<F>(&self, shutdown: F) -> Result<(), EngineError>
    where
        F: Future<Output = ()> + Send,
    {
        if self.config.inbounds.is_empty() {
            return Err(EngineError::NoInbounds);
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let mut listeners = JoinSet::new();
        let mut urltests = JoinSet::new();

        for inbound in &self.config.inbounds {
            match inbound.protocol {
                InboundProtocolConfig::Socks5 { .. } => {
                    #[cfg(feature = "inbound-socks5")]
                    {
                        let engine = self.clone();
                        let inbound = inbound.clone();
                        let shutdown = shutdown_rx.clone();
                        listeners.spawn(async move {
                            engine.run_socks5_listener(inbound, shutdown).await
                        });
                    }
                    #[cfg(not(feature = "inbound-socks5"))]
                    {
                        return Err(EngineError::CompiledFeatureDisabled {
                            kind: "inbound",
                            tag: inbound.tag.clone(),
                            protocol: "socks5",
                            feature: "inbound-socks5",
                        });
                    }
                }
                InboundProtocolConfig::HttpConnect => {
                    #[cfg(feature = "inbound-http-connect")]
                    {
                        let engine = self.clone();
                        let inbound = inbound.clone();
                        let shutdown = shutdown_rx.clone();
                        listeners.spawn(async move {
                            engine.run_http_connect_listener(inbound, shutdown).await
                        });
                    }
                    #[cfg(not(feature = "inbound-http-connect"))]
                    {
                        return Err(EngineError::CompiledFeatureDisabled {
                            kind: "inbound",
                            tag: inbound.tag.clone(),
                            protocol: "http-connect",
                            feature: "inbound-http-connect",
                        });
                    }
                }
                InboundProtocolConfig::Mixed { .. } => {
                    #[cfg(feature = "inbound-mixed")]
                    {
                        let engine = self.clone();
                        let inbound = inbound.clone();
                        let shutdown = shutdown_rx.clone();
                        listeners.spawn(async move {
                            engine.run_mixed_listener(inbound, shutdown).await
                        });
                    }
                    #[cfg(not(feature = "inbound-mixed"))]
                    {
                        return Err(EngineError::CompiledFeatureDisabled {
                            kind: "inbound",
                            tag: inbound.tag.clone(),
                            protocol: "mixed",
                            feature: "inbound-mixed",
                        });
                    }
                }
            }
        }

        for &group_id in self.plan.urltest_groups() {
            let engine = self.clone();
            let shutdown = shutdown_rx.clone();
            urltests.spawn(async move { engine.run_urltest_group(group_id, shutdown).await });
        }

        info!(
            inbound_count = self.config.inbounds.len(),
            outbound_count = self.config.outbounds.len(),
            outbound_group_count = self.config.outbound_groups.len(),
            rule_count = self.config.route.rules.len(),
            mode = %self.config.mode.kind(),
            udp_upstream_idle_timeout_seconds = self.udp_upstream_idle_timeout().as_secs(),
            supported_inbounds = ?self.protocols.supported_inbounds(),
            supported_outbounds = ?self.protocols.supported_outbounds(),
            "zero-engine started"
        );

        tokio::pin!(shutdown);
        let mut shutting_down = false;

        loop {
            if shutting_down && listeners.is_empty() && urltests.is_empty() {
                let stats = self.stats_snapshot();
                info!(
                    total_started = stats.total_started,
                    completed_sessions = stats.completed_sessions,
                    failed_sessions = stats.failed_sessions,
                    blocked_sessions = stats.blocked_sessions,
                    direct_sessions = stats.direct_sessions,
                    chained_sessions = stats.chained_sessions,
                    udp_upstream_active_associations = stats.udp_upstream.active_associations,
                    udp_upstream_created_associations = stats.udp_upstream.created_associations,
                    udp_upstream_reused_associations = stats.udp_upstream.reused_associations,
                    udp_upstream_closed_associations = stats.udp_upstream.closed_associations,
                    udp_upstream_idle_timeouts = stats.udp_upstream.idle_timeouts,
                    udp_upstream_dropped_associations = stats.udp_upstream.dropped_associations,
                    "zero-engine stopped"
                );
                return Ok(());
            }

            tokio::select! {
                _ = &mut shutdown, if !shutting_down => {
                    shutting_down = true;
                    let _ = shutdown_tx.send(true);
                    info!("propagated engine shutdown to background tasks");
                }
                result = listeners.join_next(), if !listeners.is_empty() => {
                    match result {
                        Some(Ok(Ok(()))) if shutting_down => {}
                        Some(Ok(Ok(()))) => return Err(EngineError::InboundTaskExited),
                        Some(Ok(Err(error))) => return Err(error),
                        Some(Err(error)) => return Err(io::Error::other(error).into()),
                        None if shutting_down => return Ok(()),
                        None => return Err(EngineError::InboundTaskExited),
                    }
                }
                result = urltests.join_next(), if !urltests.is_empty() => {
                    match result {
                        Some(Ok(Ok(()))) if shutting_down => {}
                        Some(Ok(Ok(()))) => return Err(EngineError::UrlTestTaskExited),
                        Some(Ok(Err(error))) => return Err(error),
                        Some(Err(error)) => return Err(io::Error::other(error).into()),
                        None if shutting_down => {}
                        None => return Err(EngineError::UrlTestTaskExited),
                    }
                }
            }
        }
    }

    pub(crate) fn resolve_outbound<'a>(
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

    fn resolve_target<'a>(&'a self, tag: &'a str) -> Result<ResolvedOutbound<'a>, EngineError> {
        let Some(target_id) = self.plan.target_id(tag) else {
            return Err(EngineError::MissingRouteTarget {
                tag: tag.to_owned(),
            });
        };

        resolve_target_id(&self.plan, &self.outbound_group_state, target_id).ok_or_else(|| {
            EngineError::MissingRouteTarget {
                tag: tag.to_owned(),
            }
        })
    }

    #[cfg(feature = "outbound-socks5")]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &zero_core::Session,
        server: &str,
        port: u16,
        auth: Option<(&str, &str)>,
    ) -> Result<TokioSocket, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;
        let mut upstream = super::metered::MeteredStream::new(upstream);

        self.protocols
            .socks5_outbound
            .establish_tunnel_with_auth(
                &mut upstream,
                session,
                auth.map(
                    |(username, password)| zero_protocol_socks5::Socks5OutboundAuth {
                        username,
                        password,
                    },
                ),
            )
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner())
    }

    #[cfg(not(feature = "outbound-socks5"))]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        _session: &zero_core::Session,
        _server: &str,
        _port: u16,
        _auth: Option<(&str, &str)>,
    ) -> Result<TokioSocket, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "socks5-upstream".to_owned(),
            protocol: "socks5",
            feature: "outbound-socks5",
        })
    }

    pub(crate) fn prepare_session(&self, session: &mut Session, inbound_tag: &str) {
        session.id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        session.inbound_tag = Some(inbound_tag.to_owned());
        self.session_registry
            .insert(session, self.config.mode.kind());
        self.stats.record_start();
    }

    pub(crate) fn set_session_outbound(&self, session: &Session) {
        self.session_registry
            .update_outbound_tag(session.id, session.outbound_tag.as_deref());
    }

    pub(crate) fn record_session_upload(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_upload(session_id, bytes);
    }

    pub(crate) fn record_session_download(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_download(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_rx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_inbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_tx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_inbound_tx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_rx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_outbound_rx(session_id, bytes);
    }

    pub(crate) fn record_session_outbound_tx(&self, session_id: u64, bytes: u64) {
        self.session_registry.record_outbound_tx(session_id, bytes);
    }

    pub(crate) fn record_session_inbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_inbound_rx(session_id, traffic.read_bytes);
        self.record_session_inbound_tx(session_id, traffic.written_bytes);
    }

    pub(crate) fn record_session_outbound_traffic(&self, session_id: u64, traffic: StreamTraffic) {
        if traffic.is_empty() {
            return;
        }

        self.record_session_outbound_rx(session_id, traffic.read_bytes);
        self.record_session_outbound_tx(session_id, traffic.written_bytes);
    }

    pub(crate) fn finish_session(
        &self,
        session_id: u64,
        outcome: SessionOutcome,
    ) -> Option<CompletedSessionRecord> {
        let record = self.session_registry.finish(session_id, outcome)?;
        self.stats.record_finish(outcome);
        self.completed_sessions.push(record.clone());
        Some(record)
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        SessionHandle::new(self.clone(), session_id)
    }
}

pub(crate) async fn bind_listener(inbound: &InboundConfig) -> io::Result<TokioListener> {
    let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    TokioListener::bind(&listen).await
}
