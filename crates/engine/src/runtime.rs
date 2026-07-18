use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use tracing::info;
use zero_config::{ModeConfig, RuntimeConfig};
use zero_core::Address;
use zero_router::{RouteAction, RouteContext, RuleSet};

use super::completed_sessions::CompletedSessionHistory;
use super::error::EngineError;
use super::event_log::EngineEventLog;
use super::groups::OutboundGroupStateStore;
use super::hook::{FlowHook, FlowHookChain};
use super::outbound_health::OutboundHealth;
use super::passive_relay_health::PassiveRelayHealth;
use super::plan::{EnginePlan, TargetId};
use super::probe_trigger::ProbeTriggerRegistry;
use super::resolve::{
    resolve_target_chains, resolve_target_id, ResolvedLeafOutbound, ResolvedOutbound,
};
use super::session_registry::SessionRegistry;
use super::stats::EngineStats;

mod configuration;
mod diagnostics;
mod observability;
mod passive_health;
mod policy;
mod session;

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
    pub(crate) passive_relay_health: Arc<PassiveRelayHealth>,
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
    fn into_route_action(self) -> RouteAction {
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
            passive_relay_health: Arc::new(PassiveRelayHealth::default()),
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
        self.route_decision(address, None).into_route_action()
    }

    pub fn route_decision(&self, address: &Address, sni: Option<&str>) -> RouteDecision {
        self.route_decision_with_inbound(address, sni, None)
    }

    pub fn route_decision_with_inbound(
        &self,
        address: &Address,
        sni: Option<&str>,
        inbound_tag: Option<&str>,
    ) -> RouteDecision {
        let mode = self.mode.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match &mode {
            ModeConfig::Rule => {
                let action = self
                    .router
                    .lock()
                    .expect("router lock poisoned")
                    .decide_with_context(RouteContext {
                        address,
                        sni,
                        inbound_tag,
                    });
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
}
