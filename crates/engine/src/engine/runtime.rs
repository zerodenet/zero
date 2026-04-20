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
use super::resolve::{resolve_named_outbound, resolve_selector_group, ResolvedOutbound};
use super::session_lifecycle::SessionHandle;
use super::session_registry::{ActiveSession, SessionRegistry};
use super::stats::{EngineStats, EngineStatsSnapshot, SessionOutcome};

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) config: RuntimeConfig,
    pub(crate) router: RuleSet,
    pub(crate) resolver: TokioResolver,
    pub(crate) protocols: ProtocolInventory,
    pub(crate) next_session_id: Arc<AtomicU64>,
    pub(crate) session_registry: Arc<SessionRegistry>,
    pub(crate) completed_sessions: Arc<CompletedSessionHistory>,
    pub(crate) stats: Arc<EngineStats>,
    pub(crate) udp_upstream_idle_timeout: Duration,
}

impl Engine {
    pub fn new(config: RuntimeConfig) -> Result<Self, EngineError> {
        let router = config.route.compile()?;
        let udp_upstream_idle_timeout =
            Duration::from_secs(config.runtime.udp_upstream_idle_timeout_seconds);

        Ok(Self {
            config,
            router,
            resolver: TokioResolver,
            protocols: ProtocolInventory::default(),
            next_session_id: Arc::new(AtomicU64::new(1)),
            session_registry: SessionRegistry::shared(),
            completed_sessions: CompletedSessionHistory::shared(),
            stats: EngineStats::shared(),
            udp_upstream_idle_timeout,
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let config = RuntimeConfig::load_from_path(path)?;
        Self::new(config)
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
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
        match &self.config.mode {
            ModeConfig::Rule => self.router.decide(address),
            ModeConfig::Direct => RouteAction::Direct,
            ModeConfig::Global { outbound } => RouteAction::Route(outbound.clone()),
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

        for inbound in &self.config.inbounds {
            match inbound.protocol {
                InboundProtocolConfig::Socks5 => {
                    let engine = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners
                        .spawn(async move { engine.run_socks5_listener(inbound, shutdown).await });
                }
                InboundProtocolConfig::HttpConnect => {
                    let engine = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners.spawn(async move {
                        engine.run_http_connect_listener(inbound, shutdown).await
                    });
                }
                InboundProtocolConfig::Mixed => {
                    let engine = self.clone();
                    let inbound = inbound.clone();
                    let shutdown = shutdown_rx.clone();
                    listeners
                        .spawn(async move { engine.run_mixed_listener(inbound, shutdown).await });
                }
            }
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
            if shutting_down && listeners.is_empty() {
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
                    info!("propagated engine shutdown to inbound listeners");
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
            }
        }
    }

    pub(crate) fn resolve_outbound<'a>(
        &'a self,
        action: &'a RouteAction,
    ) -> Result<ResolvedOutbound<'a>, EngineError> {
        match action {
            RouteAction::Direct => Ok(ResolvedOutbound::Direct { tag: None }),
            RouteAction::Reject => Ok(ResolvedOutbound::Block { tag: None }),
            RouteAction::Route(tag) => self.resolve_target(tag),
        }
    }

    fn resolve_target<'a>(&'a self, tag: &'a str) -> Result<ResolvedOutbound<'a>, EngineError> {
        if let Some(outbound) = self
            .config
            .outbounds
            .iter()
            .find(|outbound| outbound.tag() == tag)
        {
            return Ok(resolve_named_outbound(outbound));
        }

        if let Some(group) = self
            .config
            .outbound_groups
            .iter()
            .find(|group| group.tag() == tag)
        {
            return resolve_selector_group(group, &self.config.outbounds).ok_or_else(|| {
                EngineError::MissingRouteTarget {
                    tag: tag.to_owned(),
                }
            });
        }

        Err(EngineError::MissingRouteTarget {
            tag: tag.to_owned(),
        })
    }

    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &zero_core::Session,
        server: &str,
        port: u16,
    ) -> Result<TokioSocket, EngineError> {
        let mut upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;

        self.protocols
            .socks5_outbound
            .establish_tunnel(&mut upstream, session)
            .await?;

        Ok(upstream)
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
