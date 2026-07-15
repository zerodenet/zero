use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_engine::EngineError;

use super::{listeners, reload, Proxy};
use crate::runtime::route_runtime::{InboundListenerRuntimeFactory, SharedIngressRuntimeServices};

pub(super) async fn run_until<F>(proxy: &Proxy, shutdown: F) -> Result<(), EngineError>
where
    F: Future<Output = ()> + Send,
{
    if proxy.config.inbounds.is_empty() {
        return Err(EngineError::NoInbounds);
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut listeners: JoinSet<Result<(), EngineError>> = JoinSet::new();
    let mut listener_stops: HashMap<String, watch::Sender<bool>> = HashMap::new();
    let mut urltests: JoinSet<Result<(), EngineError>> = JoinSet::new();
    let mut reload_async_rx = reload::subscribe_reload_bridge(proxy.engine.subscribe_reload());
    let source_dir: Option<PathBuf> = proxy.config.source_dir().map(|path| path.to_path_buf());
    let inbound_runtime_factory =
        InboundListenerRuntimeFactory::new(SharedIngressRuntimeServices::from_proxy(proxy));

    for inbound in &proxy.config.inbounds {
        let (tx, rx) = watch::channel(false);
        listener_stops.insert(inbound.tag.clone(), tx);
        let bound =
            listeners::bind_inbound_listener(&proxy.protocols, source_dir.as_deref(), inbound)
                .await?;
        listeners::spawn_inbound_listener(
            &proxy.protocols,
            source_dir.as_deref(),
            &inbound_runtime_factory,
            inbound,
            bound,
            rx,
            &mut listeners,
        );
    }
    for &group_id in proxy.engine.plan().urltest_groups() {
        let proxy = proxy.clone();
        let shutdown = shutdown_rx.clone();
        urltests.spawn(async move { proxy.run_urltest_group(group_id, shutdown).await });
    }

    info!(
        inbound_count = proxy.config.inbounds.len(),
        outbound_count = proxy.config.outbounds.len(),
        outbound_group_count = proxy.config.outbound_groups.len(),
        rule_count = proxy.config.route.rules.len(),
        mode = %proxy.config.mode.kind(),
        udp_upstream_idle_timeout_seconds = proxy.engine.udp_upstream_idle_timeout().as_secs(),
        supported_inbounds = ?proxy.protocols.supported_inbounds(),
        supported_outbounds = ?proxy.protocols.supported_outbounds(),
        "zero-proxy started"
    );

    tokio::pin!(shutdown);
    let mut shutting_down = false;

    loop {
        if shutting_down && listeners.is_empty() && urltests.is_empty() {
            log_stopped(proxy);
            return Ok(());
        }

        tokio::select! {
            _ = &mut shutdown, if !shutting_down => {
                shutting_down = true;
                let _ = shutdown_tx.send(true);
                for tx in listener_stops.values() {
                    let _ = tx.send(true);
                }
                info!("propagated proxy shutdown to background tasks");
            }
            Some(()) = reload_async_rx.recv() => {
                if shutting_down {
                    continue;
                }
                let new_config = proxy.engine.config();
                if let Err(error) = proxy.resolver.reload(new_config.runtime.dns.as_ref()) {
                    warn!(%error, "failed to reload dns config");
                }
                listeners::reconcile_inbounds(
                    &proxy.protocols,
                    source_dir.as_deref(),
                    &inbound_runtime_factory,
                    &new_config,
                    &mut listener_stops,
                    &mut listeners,
                ).await;
                listeners::reconcile_urltests(proxy, &new_config, &shutdown_rx, &mut urltests).await;
                proxy.protocols.on_config_reloaded();
                info!(
                    inbound_count = new_config.inbounds.len(),
                    outbound_count = new_config.outbounds.len(),
                    outbound_group_count = new_config.outbound_groups.len(),
                    "config reload reconciled"
                );
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

fn log_stopped(proxy: &Proxy) {
    let stats = proxy.engine.stats_snapshot();
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
        "zero-proxy stopped"
    );
}
