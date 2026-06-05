use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};
use zero_api::{EventFilter, RawApiEvent};
use zero_config::{ModeConfig, RuntimeConfig};
use zero_core::{Address, Session};
use zero_router::{RouteAction, RuleSet};

use super::completed_sessions::{CompletedSessionHistory, CompletedSessionRecord};
use super::error::EngineError;
use super::event_log::EngineEventLog;
use super::groups::{OutboundGroupStateStore, UrlTestGroupState, UrlTestMemberState};
use super::hook::{FlowHook, FlowHookChain};
use super::outbound_health::OutboundHealth;
use super::plan::{EnginePlan, TargetId};
use super::probe_trigger::ProbeTriggerRegistry;
use super::resolve::{
    resolve_target_chains, resolve_target_id, ResolvedLeafOutbound, ResolvedOutbound,
};
use super::session_lifecycle::SessionHandle;
use super::session_registry::{ActiveSession, SessionRegistry};
use super::stats::{EngineStats, SessionOutcome};
use super::view::PlanView;

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) config: Arc<std::sync::RwLock<Arc<RuntimeConfig>>>,
    pub(crate) plan: Arc<std::sync::Mutex<Arc<EnginePlan>>>,
    pub(crate) router: Arc<std::sync::Mutex<Arc<RuleSet>>>,
    mode: Arc<std::sync::Mutex<ModeConfig>>,
    next_session_id: Arc<AtomicU64>,
    session_registry: Arc<SessionRegistry>,
    completed_sessions: Arc<CompletedSessionHistory>,
    event_log: Arc<EngineEventLog>,
    stats: Arc<EngineStats>,
    pub(crate) outbound_group_state: Arc<OutboundGroupStateStore>,
    pub(crate) probe_trigger_registry: Arc<ProbeTriggerRegistry>,
    flow_hook: Option<Arc<FlowHookChain>>,
    pub(crate) outbound_health: Arc<OutboundHealth>,
    udp_upstream_idle_timeout: Duration,
    /// Reload notification channel: wakes the proxy's main loop when
    /// `reload_config` atomically swaps the plan / router / config.
    reload_notify: Arc<std::sync::Mutex<Vec<std::sync::mpsc::Sender<()>>>>,
    /// Source path of the running config.  When set, `reload_config`
    /// writes the new config back to this path so it survives restarts.
    config_path: Option<std::path::PathBuf>,
    /// Process start time (UNIX epoch milliseconds), captured on Engine::new.
    pub(crate) started_at_unix_ms: u64,
    /// ID of the OS process hosting this engine.
    pub(crate) pid: u32,
    /// External sink status injected by the event dispatcher.
    /// Updated via `update_sink_status()` when the dispatcher runs.
    sink_status: Arc<std::sync::Mutex<Vec<zero_api::SinkStatus>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    Route(String),
    Direct,
    Reject,
}

impl RouteDecision {
    fn to_owned(self) -> RouteAction {
        match self {
            Self::Route(tag) => RouteAction::Route(tag),
            Self::Direct => RouteAction::Direct,
            Self::Reject => RouteAction::Reject,
        }
    }
}

impl From<&RouteAction> for RouteDecision {
    fn from(value: &RouteAction) -> Self {
        match value {
            RouteAction::Route(tag) => Self::Route(tag.clone()),
            RouteAction::Direct => Self::Direct,
            RouteAction::Reject => Self::Reject,
        }
    }
}

/// Current UNIX epoch in milliseconds.
fn started_at_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Engine {
    pub fn new(config: RuntimeConfig) -> Result<Self, EngineError> {
        let router = Arc::new(std::sync::Mutex::new(Arc::new(
            config.route.compile(config.source_dir())?,
        )));
        let plan = Arc::new(std::sync::Mutex::new(Arc::new(EnginePlan::build(&config)?)));
        let plan_inner = plan.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let udp_upstream_idle_timeout =
            Duration::from_secs(config.runtime.udp_upstream_idle_timeout_seconds);
        let outbound_group_state = OutboundGroupStateStore::shared();

        for &group_id in plan_inner.selector_groups() {
            let group = plan_inner
                .target(group_id)
                .expect("engine plan should resolve selector group");
            let Some(selector) = group.as_selector() else {
                continue;
            };
            outbound_group_state.initialize_selector(group_id, selector.initial_member());
        }

        for &group_id in plan_inner.urltest_groups() {
            let group = plan_inner
                .target(group_id)
                .expect("engine plan should resolve urltest group");
            let Some(urltest) = group.as_urltest() else {
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

        for &group_id in plan_inner.loadbalance_groups() {
            outbound_group_state.initialize_loadbalance(group_id);
        }

        let event_log = EngineEventLog::shared();

        info!(build_id = env!("CARGO_PKG_VERSION"), "engine started");
        event_log.push_engine_started(env!("CARGO_PKG_VERSION"));

        let mode = Arc::new(std::sync::Mutex::new(config.mode.clone()));
        Ok(Self {
            config: Arc::new(std::sync::RwLock::new(Arc::new(config))),
            mode,
            plan,
            router,
            next_session_id: Arc::new(AtomicU64::new(1)),
            session_registry: SessionRegistry::shared(),
            completed_sessions: CompletedSessionHistory::shared(),
            event_log,
            stats: EngineStats::shared(),
            outbound_group_state,
            probe_trigger_registry: ProbeTriggerRegistry::shared(),
            outbound_health: Arc::new(OutboundHealth::new()),
            flow_hook: None,
            udp_upstream_idle_timeout,
            reload_notify: Arc::new(std::sync::Mutex::new(Vec::new())),
            config_path: None,
            started_at_unix_ms: started_at_unix_ms(),
            pid: std::process::id(),
            sink_status: Arc::new(std::sync::Mutex::new(Vec::new())),
        })
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let config = RuntimeConfig::load_from_path(path.as_ref())?;
        let mut engine = Self::new(config)?;
        engine.config_path = Some(path.as_ref().to_owned());
        Ok(engine)
    }

    pub fn config(&self) -> Arc<RuntimeConfig> {
        self.config.read().expect("config lock poisoned").clone()
    }

    /// The config file path used to start or reload this engine.
    pub fn config_path(&self) -> Option<&std::path::Path> {
        self.config_path.as_deref()
    }

    /// UNIX epoch milliseconds when this engine was created.
    pub fn started_at_unix_ms(&self) -> u64 {
        self.started_at_unix_ms
    }

    pub fn plan(&self) -> Arc<EnginePlan> {
        self.plan.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn with_udp_upstream_idle_timeout(mut self, timeout: Duration) -> Self {
        self.udp_upstream_idle_timeout = timeout;
        self
    }

    pub fn with_flow_hook(mut self, hook: impl FlowHook + 'static) -> Self {
        let mut chain = FlowHookChain::empty();
        chain.push(Arc::new(hook));
        self.flow_hook = Some(Arc::new(chain));
        self
    }

    pub fn with_flow_hook_chain(mut self, chain: FlowHookChain) -> Self {
        if !chain.is_empty() {
            self.flow_hook = Some(Arc::new(chain));
        }
        self
    }

    pub fn udp_upstream_idle_timeout(&self) -> Duration {
        self.udp_upstream_idle_timeout
    }

    pub fn mode_kind(&self) -> &'static str {
        self.mode.lock().unwrap_or_else(|e| e.into_inner()).kind()
    }

    pub fn current_mode(&self) -> ModeConfig {
        self.mode.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Atomically switch the global proxy mode at runtime.
    pub fn set_mode(&self, new_mode: ModeConfig) {
        let mut mode = self.mode.lock().unwrap_or_else(|e| e.into_inner());
        *mode = new_mode.clone();
        self.event_log.push_config_changed();
        info!(mode = new_mode.kind(), "proxy mode switched");
    }

    pub fn route_for(&self, address: &Address) -> RouteAction {
        self.route_decision(address, None).to_owned()
    }

    pub fn route_decision(&self, address: &Address, sni: Option<&str>) -> RouteDecision {
        let mode = self.mode.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match &mode {
            ModeConfig::Rule => {
                let action = self
                    .router
                    .lock()
                    .expect("router lock poisoned")
                    .decide(address, sni);
                match action {
                    RouteAction::Route(tag) => RouteDecision::Route(tag),
                    RouteAction::Direct => RouteDecision::Direct,
                    RouteAction::Reject => RouteDecision::Reject,
                }
            }
            ModeConfig::Direct => RouteDecision::Direct,
            ModeConfig::Global { outbound } => RouteDecision::Route(outbound.clone()),
        }
    }

    pub fn resolve_route_decision(
        &self,
        action: RouteDecision,
    ) -> Result<(ResolvedOutbound<'static>, Option<Arc<EnginePlan>>), EngineError> {
        match action {
            RouteDecision::Direct => Ok((
                ResolvedOutbound::Single(ResolvedLeafOutbound::Direct { tag: None }),
                None,
            )),
            RouteDecision::Reject => Ok((
                ResolvedOutbound::Single(ResolvedLeafOutbound::Block { tag: None }),
                None,
            )),
            RouteDecision::Route(tag) => {
                let (resolved, plan) = self.resolve_target(&tag)?;
                Ok((resolved, Some(plan)))
            }
        }
    }

    pub fn resolve_route_action(
        &self,
        action: &RouteAction,
    ) -> Result<(ResolvedOutbound<'static>, Option<Arc<EnginePlan>>), EngineError> {
        self.resolve_route_decision(action.into())
    }

    pub fn resolve_target_id(
        &self,
        target_id: TargetId,
    ) -> Option<(ResolvedOutbound<'static>, Arc<EnginePlan>)> {
        let plan = self.plan();
        // SAFETY: plan is returned in the tuple.  The resolved outbound
        // borrows from data inside `plan`, which stays alive as long as
        // the caller holds the returned `Arc<EnginePlan>`.
        let resolved: ResolvedOutbound<'static> = unsafe {
            std::mem::transmute(resolve_target_id(
                &plan,
                &self.outbound_group_state,
                target_id,
            )?)
        };
        Some((resolved, plan))
    }

    pub fn resolve_target_chains(&self, target_id: TargetId) -> Vec<Vec<TargetId>> {
        let plan = self.plan();
        resolve_target_chains(&plan, &self.outbound_group_state, target_id)
    }

    pub fn target_tag(&self, target_id: TargetId) -> Option<String> {
        let plan = self.plan();
        plan.target(target_id).map(|target| target.tag().to_owned())
    }

    fn resolve_target(
        &self,
        tag: &str,
    ) -> Result<(ResolvedOutbound<'static>, Arc<EnginePlan>), EngineError> {
        let plan = self.plan();
        let Some(target_id) = plan.target_id(tag) else {
            return Err(EngineError::MissingRouteTarget {
                tag: tag.to_owned(),
            });
        };
        // SAFETY: plan is returned alongside, keeping data alive.
        let resolved: ResolvedOutbound<'static> = unsafe {
            std::mem::transmute(
                resolve_target_id(&plan, &self.outbound_group_state, target_id).ok_or_else(
                    || EngineError::MissingRouteTarget {
                        tag: tag.to_owned(),
                    },
                )?,
            )
        };
        Ok((resolved, plan))
    }

    pub fn stats_snapshot(&self) -> zero_api::StatsSnapshot {
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

    pub fn events_since(
        &self,
        since: u64,
        limit: usize,
        filter: &EventFilter,
    ) -> super::event_log::EventsSinceResult {
        self.event_log.events_since(since, limit, filter)
    }

    pub fn latest_event_sequence(&self) -> u64 {
        self.event_log.latest_sequence()
    }

    pub fn push_stats_sampled(&self) {
        let snapshot = self.stats_snapshot();
        self.event_log.push_stats_sampled(&snapshot);
    }

    /// Emit an `engine.stopped` event during graceful shutdown.
    pub fn push_engine_stopped(&self, reason: &str) {
        self.event_log.push_engine_stopped(reason);
    }

    /// Update the external sink status snapshot (called by the event dispatcher).
    pub fn update_sink_status(&self, status: Vec<zero_api::SinkStatus>) {
        *self.sink_status.lock().expect("sink status lock poisoned") = status;
    }

    /// Read the current sink status snapshot.
    pub fn sink_status_snapshot(&self) -> zero_api::SinkStatusSnapshot {
        zero_api::SinkStatusSnapshot {
            sinks: self
                .sink_status
                .lock()
                .expect("sink status lock poisoned")
                .clone(),
        }
    }

    /// Hot-reload route rules and outbound group definitions.
    ///
    /// Both the router and plan are rebuilt from the new config and
    /// swapped atomically.  Active connections keep using the old plan
    /// via their held `Arc<EnginePlan>`.  A `config.changed` event is
    /// emitted.
    pub fn reload_config(&self, new_config: RuntimeConfig) -> Result<(), EngineError> {
        let new_router = Arc::new(new_config.route.compile(new_config.source_dir())?);
        let new_plan = Arc::new(EnginePlan::build(&new_config)?);

        // Atomically swap router, plan, mode, and config.
        {
            let mut guard = self.router.lock().unwrap_or_else(|e| e.into_inner());
            *guard = new_router;
        }
        {
            let mut guard = self.plan.lock().unwrap_or_else(|e| e.into_inner());
            *guard = new_plan;
        }
        {
            let mut guard = self.mode.lock().unwrap_or_else(|e| e.into_inner());
            *guard = new_config.mode.clone();
        }
        let config_for_persist = new_config.clone();
        {
            let mut guard = self.config.write().expect("config lock poisoned");
            *guard = Arc::new(new_config);
        }

        self.event_log.push_config_changed();

        // Write the new config back to disk so it survives node restarts.
        if let Some(ref path) = self.config_path {
            if let Err(e) = write_config_to_file(path, &config_for_persist) {
                warn!(%e, path = %path.display(), "failed to persist config after reload");
            } else {
                info!(path = %path.display(), "config persisted");
            }
        }

        // Wake every subscriber so the proxy layer can reconcile listeners.
        let notify = self
            .reload_notify
            .lock()
            .expect("reload notify lock poisoned");
        for tx in notify.iter() {
            let _ = tx.send(());
        }

        Ok(())
    }

    /// Subscribe to reload notifications.
    ///
    /// Returns a receiver that gets a unit value each time `reload_config`
    /// completes successfully.  The subscriber should re-read the new
    /// config from `self.config()`.
    pub fn subscribe_reload(&self) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.reload_notify
            .lock()
            .expect("reload notify lock poisoned")
            .push(tx);
        rx
    }

    /// Emit a `policy.probe.completed` event.
    pub fn push_policy_probe_completed(
        &self,
        policy_tag: &str,
        payload: zero_api::PolicyProbeCompletedPayload,
    ) {
        self.event_log
            .push_policy_probe_completed(policy_tag, payload);
    }

    /// Emit an `engine.warning` event.
    pub fn emit_warning(&self, code: &str, message: &str) {
        self.event_log.push_warning(code, message);
    }

    /// Emit `flow.updated` events for all currently active sessions.
    pub fn push_flow_updates(&self) {
        for session in self.active_sessions() {
            self.event_log.push_flow_updated(&session);
        }
    }

    pub fn set_selector_target(
        &self,
        group_tag: &str,
        target_tag: &str,
    ) -> Result<(), EngineError> {
        let plan = self.plan();
        let group_id =
            plan.target_id(group_tag)
                .ok_or_else(|| EngineError::SelectorGroupNotFound {
                    tag: group_tag.to_owned(),
                })?;
        let group = plan
            .target(group_id)
            .expect("engine plan should resolve selector group");
        let Some(selector) = group.as_selector() else {
            return Err(EngineError::SelectorGroupTypeMismatch {
                tag: group_tag.to_owned(),
            });
        };
        let target_id =
            plan.target_id(target_tag)
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
        let view = PlanView::new(&plan);

        let previous = self
            .outbound_group_state
            .selector_selected_target(group_id)
            .map(|target_id| view.target_tag_owned(target_id))
            .or_else(|| Some(view.target_tag_owned(selector.initial_member())));
        self.outbound_group_state
            .update_selector(group_id, target_id);
        self.event_log
            .push_policy_selected(group_tag, "selector", target_tag, previous.as_deref());
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
        use std::time::{SystemTime, UNIX_EPOCH};

        session.id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        session.inbound_tag = Some(inbound_tag.to_owned());

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Check hooks before committing.
        if let Some(ref hook) = self.flow_hook {
            let ctx = super::hook::FlowContext::from_session(
                session,
                self.mode.lock().unwrap_or_else(|e| e.into_inner()).kind(),
                now_ms,
            );
            if let Err(reason) = hook.on_flow_start(&ctx) {
                tracing::warn!(
                    flow_id = session.id,
                    reason = %reason.message,
                    "flow blocked by hook"
                );
                // Immediately finish as cancelled so the tracker won't
                // try to relay a non-existent session.
                self.stats
                    .record_finish(super::stats::SessionOutcome::Cancelled);
                return;
            }
        }

        self.session_registry.insert(
            session,
            self.mode.lock().unwrap_or_else(|e| e.into_inner()).kind(),
        );
        self.stats.record_start();
        self.event_log.push_flow_started(
            session,
            self.mode.lock().unwrap_or_else(|e| e.into_inner()).kind(),
        );
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
        self.stats.record_traffic(
            record.outbound_tag.as_deref(),
            record.bytes_up,
            record.bytes_down,
        );
        self.completed_sessions.push(record.clone());
        self.event_log
            .push_flow_completed(&record, |tag| self.outbound_protocol_for_tag(tag));

        // Notify hooks.
        if let Some(ref hook) = self.flow_hook {
            let ctx = super::hook::FlowContext::from_completed(&record);
            let stats = super::hook::FlowTraffic::from_completed(&record);
            hook.on_flow_end(&ctx, outcome, &stats);
        }

        Some(record)
    }

    pub fn track_session(&self, session_id: u64) -> SessionHandle {
        SessionHandle::new(self.clone(), session_id)
    }

    /// Check whether an outbound is healthy enough to accept connections.
    pub fn check_outbound_health(&self, tag: &str) -> Result<(), EngineError> {
        self.outbound_health.check(tag)
    }

    /// Record a failed connection attempt to an outbound.
    pub fn record_outbound_failure(&self, tag: &str) {
        self.outbound_health.record_failure(tag);
    }

    /// Clear health state for an outbound after a successful connection.
    pub fn record_outbound_success(&self, tag: &str) {
        self.outbound_health.record_success(tag);
    }

    /// Resolve a hostname via DNS and return the resolved addresses.
    pub fn dns_lookup(&self, hostname: &str) -> Result<serde_json::Value, EngineError> {
        use std::net::ToSocketAddrs;

        let addr_str = format!("{hostname}:0");
        let addrs: Vec<String> = addr_str
            .to_socket_addrs()
            .map_err(|e| EngineError::Io(std::io::Error::other(e)))?
            .map(|a| a.ip().to_string())
            .collect();

        Ok(serde_json::json!({
            "hostname": hostname,
            "resolved_addresses": addrs,
            "count": addrs.len(),
        }))
    }

    /// Walk the routing rules and return the ones that would match the given
    /// target tuple (host, port, protocol).
    pub fn trace_route(
        &self,
        target: &str,
        port: u16,
        protocol: &str,
    ) -> Result<serde_json::Value, EngineError> {
        let address = match target.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V4(v4)) => zero_core::Address::Ipv4(v4.octets()),
            Ok(std::net::IpAddr::V6(v6)) => zero_core::Address::Ipv6(v6.octets()),
            Err(_) => zero_core::Address::Domain(target.to_owned()),
        };

        let router = self.router.lock().unwrap_or_else(|e| e.into_inner());
        let action = router.decide(&address, None);

        let mode = self.mode_kind();

        Ok(serde_json::json!({
            "target": target,
            "port": port,
            "protocol": protocol,
            "effective_mode": mode,
            "route_action": match action {
                zero_router::RouteAction::Route(tag) => serde_json::json!({"route": tag}),
                zero_router::RouteAction::Direct => serde_json::json!("direct"),
                zero_router::RouteAction::Reject => serde_json::json!("reject"),
            },
        }))
    }

    /// Test TCP reachability of a target outbound by performing a short
    /// connect from the proxy's own network stack.
    pub fn probe_target(&self, target_tag: &str) -> Result<serde_json::Value, EngineError> {
        use std::net::{TcpStream, ToSocketAddrs};

        let plan = self.plan();
        let Some(target_id) = plan.target_id(target_tag) else {
            return Err(EngineError::SelectorGroupNotFound {
                tag: target_tag.to_owned(),
            });
        };
        let Some((resolved, _plan)) = self.resolve_target_id(target_id) else {
            return Err(EngineError::SelectorGroupNotFound {
                tag: target_tag.to_owned(),
            });
        };

        // Extract server:port from the resolved target.
        let (host, port) = match &resolved {
            crate::ResolvedOutbound::Single(leaf) => extract_target_addr(leaf),
            crate::ResolvedOutbound::Fallback { candidates } => match candidates.first() {
                Some(c) => extract_target_addr(c),
                None => (None, None),
            },
            crate::ResolvedOutbound::Relay { .. } => (None, None),
        };

        let (Some(host), Some(port)) = (host, port) else {
            return Ok(serde_json::json!({
                "target_tag": target_tag,
                "reachable": false,
                "error": "outbound has no probeable fixed server",
            }));
        };

        let addr = format!("{host}:{port}");
        let started = std::time::Instant::now();

        // Short timeout blocking connect.
        let addr = addr.to_socket_addrs().ok().and_then(|mut a| a.next());
        let reachable = addr
            .map(|a| TcpStream::connect_timeout(&a, std::time::Duration::from_secs(2)).is_ok())
            .unwrap_or(false);

        Ok(serde_json::json!({
            "target_tag": target_tag,
            "server": host,
            "port": port,
            "reachable": reachable,
            "latency_ms": if reachable {
                Some(started.elapsed().as_millis() as u64)
            } else {
                None
            },
        }))
    }

    /// Force-close an active flow by its flow id.
    ///
    /// Returns `Ok(())` if the flow was found and closed, or an error if
    /// the flow id is invalid or the flow is no longer active.
    pub fn probe_trigger_registry(&self) -> &ProbeTriggerRegistry {
        &self.probe_trigger_registry
    }

    /// Request an immediate urltest probe cycle for the given policy tag.
    ///
    /// Returns an error if the policy is not found or is not a urltest group.
    pub fn trigger_urltest_probe(&self, policy_tag: &str) -> Result<(), EngineError> {
        let trigger = self.probe_trigger_registry.get(policy_tag).ok_or_else(|| {
            EngineError::SelectorGroupNotFound {
                tag: policy_tag.to_owned(),
            }
        })?;
        trigger.trigger();
        Ok(())
    }

    pub fn close_flow(&self, flow_id: &str) -> Result<(), EngineError> {
        let session_id: u64 = flow_id.parse().map_err(|_| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid flow id",
            ))
        })?;
        self.finish_session(session_id, SessionOutcome::Cancelled)
            .map(|_| ())
            .ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("flow `{flow_id}` not found or already completed"),
                ))
            })
    }

    fn outbound_protocol_for_tag(&self, tag: &str) -> Option<&'static str> {
        if tag == "direct" {
            return Some("direct");
        }
        if tag == "block" {
            return Some("block");
        }

        self.config()
            .outbounds
            .iter()
            .find(|outbound| outbound.tag == tag)
            .map(|outbound| match outbound.protocol {
                zero_config::OutboundProtocolConfig::Direct => "direct",
                zero_config::OutboundProtocolConfig::Block => "block",
                zero_config::OutboundProtocolConfig::Socks5 { .. } => "socks5",
                zero_config::OutboundProtocolConfig::Vless { .. } => "vless",
                zero_config::OutboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
                zero_config::OutboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
                zero_config::OutboundProtocolConfig::Trojan { .. } => "trojan",
                zero_config::OutboundProtocolConfig::Vmess { .. } => "vmess",
                zero_config::OutboundProtocolConfig::Mieru { .. } => "mieru",
            })
    }
}

/// Extract a `(server, port)` pair from a resolved leaf outbound.
fn extract_target_addr(leaf: &crate::ResolvedLeafOutbound<'_>) -> (Option<String>, Option<u16>) {
    match leaf {
        crate::ResolvedLeafOutbound::Direct { .. } | crate::ResolvedLeafOutbound::Block { .. } => {
            (None, None)
        }
        crate::ResolvedLeafOutbound::Socks5 { server, port, .. }
        | crate::ResolvedLeafOutbound::Vless { server, port, .. }
        | crate::ResolvedLeafOutbound::Hysteria2 { server, port, .. }
        | crate::ResolvedLeafOutbound::Shadowsocks { server, port, .. }
        | crate::ResolvedLeafOutbound::Trojan { server, port, .. }
        | crate::ResolvedLeafOutbound::Vmess { server, port, .. }
        | crate::ResolvedLeafOutbound::Mieru { server, port, .. } => {
            (Some(server.to_string()), Some(*port))
        }
    }
}

fn write_config_to_file(path: &Path, config: &RuntimeConfig) -> Result<(), io::Error> {
    let json = serde_json::to_string_pretty(config).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("serialize config: {e}"))
    })?;
    std::fs::write(path, json)?;
    Ok(())
}
