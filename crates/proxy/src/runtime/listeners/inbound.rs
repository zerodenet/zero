use std::collections::HashMap;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_config::{InboundConfig, RuntimeConfig};
use zero_engine::EngineError;

use super::super::Proxy;

pub(in crate::runtime) async fn bind_inbound_listener(
    proxy: &Proxy,
    inbound: &InboundConfig,
) -> Result<crate::protocol_registry::BoundInbound, EngineError> {
    proxy
        .protocols
        .bind_inbound(inbound, proxy.config.source_dir())
        .await
}

pub(in crate::runtime) fn spawn_inbound_listener(
    proxy: &Proxy,
    inbound: &InboundConfig,
    bound: crate::protocol_registry::BoundInbound,
    shutdown_rx: watch::Receiver<bool>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
) {
    if let Err(error) = proxy
        .protocols
        .check_inbound_enabled(&inbound.protocol, &inbound.tag)
    {
        warn!(tag = %inbound.tag, error = %error, "skipping inbound listener: feature check failed");
        return;
    }

    if let Err(error) =
        proxy
            .protocols
            .spawn_inbound(proxy, inbound.clone(), bound, shutdown_rx, listeners)
    {
        warn!(tag = %inbound.tag, error = %error, "skipping inbound listener: adapter dispatch failed");
    }
}

pub(in crate::runtime) async fn reconcile_inbounds(
    proxy: &Proxy,
    new_config: &RuntimeConfig,
    listener_stops: &mut HashMap<String, watch::Sender<bool>>,
    listeners: &mut JoinSet<Result<(), EngineError>>,
) {
    let new_tags: Vec<&str> = new_config
        .inbounds
        .iter()
        .map(|item| item.tag.as_str())
        .collect();

    listener_stops.retain(|tag, shutdown| {
        if new_tags.contains(&tag.as_str()) {
            true
        } else {
            let _ = shutdown.send(true);
            info!(%tag, "signalled shutdown for removed inbound listener");
            false
        }
    });

    for inbound in &new_config.inbounds {
        if listener_stops.contains_key(&inbound.tag) {
            continue;
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        listener_stops.insert(inbound.tag.clone(), shutdown_tx);
        match bind_inbound_listener(proxy, inbound).await {
            Ok(bound) => {
                spawn_inbound_listener(proxy, inbound, bound, shutdown_rx, listeners);
                info!(tag = %inbound.tag, "started new inbound listener");
            }
            Err(error) => {
                listener_stops.remove(&inbound.tag);
                warn!(tag = %inbound.tag, error = %error, "failed to bind inbound listener");
            }
        }
    }
}
