use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_engine::EngineError;

use super::logging::{log_reload_reconciled, log_started};
use crate::groups::UrlTestRuntime;
use crate::runtime::route_runtime::{InboundListenerRuntimeFactory, SharedIngressRuntimeServices};
use crate::runtime::{listeners, reload, Proxy};

pub(super) struct OrchestrationState {
    pub(super) shutdown_tx: watch::Sender<bool>,
    pub(super) shutdown_rx: watch::Receiver<bool>,
    pub(super) listeners: JoinSet<Result<(), EngineError>>,
    pub(super) listener_stops: HashMap<String, watch::Sender<bool>>,
    pub(super) urltests: JoinSet<Result<(), EngineError>>,
    pub(super) reload_async_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
    pub(super) source_dir: Option<PathBuf>,
    pub(super) urltest_runtime: UrlTestRuntime,
    pub(super) inbound_runtime_factory: InboundListenerRuntimeFactory,
}

impl OrchestrationState {
    pub(super) async fn new(proxy: &Proxy) -> Result<Self, EngineError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let source_dir = proxy.config.source_dir().map(|path| path.to_path_buf());
        let tcp_services = proxy.tcp_runtime_services();
        let urltest_runtime = UrlTestRuntime::new(tcp_services.clone());
        let inbound_runtime_factory =
            InboundListenerRuntimeFactory::new(SharedIngressRuntimeServices::new(tcp_services));
        let mut state = Self {
            shutdown_tx,
            shutdown_rx,
            listeners: JoinSet::new(),
            listener_stops: HashMap::new(),
            urltests: JoinSet::new(),
            reload_async_rx: reload::subscribe_reload_bridge(proxy.engine.subscribe_reload()),
            source_dir,
            urltest_runtime,
            inbound_runtime_factory,
        };

        state.start_inbounds(proxy).await?;
        state.start_urltests();
        log_started(proxy);

        Ok(state)
    }

    pub(super) fn is_idle(&self) -> bool {
        self.listeners.is_empty() && self.urltests.is_empty()
    }

    pub(super) fn propagate_shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        for tx in self.listener_stops.values() {
            let _ = tx.send(true);
        }
        info!("propagated proxy shutdown to background tasks");
    }

    pub(super) async fn reconcile_reload(&mut self, proxy: &Proxy) {
        let new_config = proxy.engine.config();
        let source_dir = self.source_dir.clone();
        if let Err(error) = proxy.resolver.reload(new_config.runtime.dns.as_ref()) {
            warn!(%error, "failed to reload dns config");
        }
        listeners::reconcile_inbounds(
            &proxy.protocols,
            source_dir.as_deref(),
            &self.inbound_runtime_factory,
            &new_config,
            &mut self.listener_stops,
            &mut self.listeners,
        )
        .await;
        listeners::reconcile_urltests(&self.urltest_runtime, &self.shutdown_rx, &mut self.urltests)
            .await;
        proxy.protocols.on_config_reloaded();
        log_reload_reconciled(&new_config);
    }

    async fn start_inbounds(&mut self, proxy: &Proxy) -> Result<(), EngineError> {
        let source_dir = self.source_dir.clone();
        for inbound in &proxy.config.inbounds {
            let (tx, rx) = watch::channel(false);
            self.listener_stops.insert(inbound.tag.clone(), tx);
            let bound =
                listeners::bind_inbound_listener(&proxy.protocols, source_dir.as_deref(), inbound)
                    .await?;
            listeners::spawn_inbound_listener(
                &proxy.protocols,
                source_dir.as_deref(),
                &self.inbound_runtime_factory,
                inbound,
                bound,
                rx,
                &mut self.listeners,
            );
        }
        Ok(())
    }

    fn start_urltests(&mut self) {
        for group_id in self.urltest_runtime.group_ids() {
            let runtime = self.urltest_runtime.clone();
            let shutdown = self.shutdown_rx.clone();
            self.urltests
                .spawn(async move { runtime.run_urltest_group(group_id, shutdown).await });
        }
    }
}
