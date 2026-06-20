use std::collections::HashMap;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_config::{InboundConfig, RuntimeConfig};
use zero_engine::EngineError;

use super::Proxy;

/// Eagerly bind a listener socket via the protocol's registered adapter.
pub(super) async fn bind_inbound_listener(
    proxy: &Proxy,
    inbound: &InboundConfig,
) -> Result<crate::protocol_adapter::BoundInbound, EngineError> {
    proxy
        .protocols
        .bind_inbound(inbound, proxy.config.source_dir())
        .await
}

pub(super) fn spawn_inbound_listener(
    proxy: &Proxy,
    inbound: &InboundConfig,
    bound: crate::protocol_adapter::BoundInbound,
    shutdown_rx: watch::Receiver<bool>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
) {
    if let Err(e) = proxy
        .protocols
        .check_inbound_enabled(&inbound.protocol, &inbound.tag)
    {
        warn!(tag = %inbound.tag, error = %e, "skipping inbound listener: feature check failed");
        return;
    }

    match proxy.protocols.find_inbound(&inbound.protocol) {
        Ok(adapter) => adapter.spawn_inbound(proxy, inbound.clone(), bound, shutdown_rx, listeners),
        Err(_) => {
            // The feature check above already validated compilation; reaching
            // here means an unregistered config.
        }
    }
}

/// Stop removed listeners through per-listener shutdown channels and start new
/// listeners through the registered adapter path.
pub(super) async fn reconcile_inbounds(
    proxy: &Proxy,
    new_config: &RuntimeConfig,
    listener_stops: &mut HashMap<String, watch::Sender<bool>>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
) {
    let new_tags: Vec<&str> = new_config.inbounds.iter().map(|i| i.tag.as_str()).collect();

    listener_stops.retain(|tag, tx| {
        if new_tags.contains(&tag.as_str()) {
            true
        } else {
            let _ = tx.send(true);
            info!(%tag, "signalled shutdown for removed inbound listener");
            false
        }
    });

    for inbound in &new_config.inbounds {
        if !listener_stops.contains_key(&inbound.tag) {
            let (tx, rx) = watch::channel(false);
            listener_stops.insert(inbound.tag.clone(), tx);
            match bind_inbound_listener(proxy, inbound).await {
                Ok(bound) => {
                    spawn_inbound_listener(proxy, inbound, bound, rx, listeners);
                    info!(tag = %inbound.tag, "started new inbound listener");
                }
                Err(e) => {
                    listener_stops.remove(&inbound.tag);
                    warn!(tag = %inbound.tag, error = %e, "failed to bind inbound listener");
                }
            }
        }
    }
}

/// Stop removed urltest groups and start new ones.
pub(super) fn reconcile_urltests(
    proxy: &Proxy,
    _new_config: &RuntimeConfig,
    shutdown_rx: &watch::Receiver<bool>,
    urltests: &mut JoinSet<Result<(), EngineError>>,
) {
    let plan = proxy.engine.plan();
    let new_ids: Vec<zero_engine::TargetId> = plan.urltest_groups().to_vec();

    for &group_id in &new_ids {
        let p = proxy.clone();
        let s = shutdown_rx.clone();
        urltests.spawn(async move { p.run_urltest_group(group_id, s).await });
    }
}
