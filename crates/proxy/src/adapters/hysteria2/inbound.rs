mod request;
mod transport;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use request::Hysteria2InboundListenerRequest;
pub(crate) use transport::run_hysteria2_listener_with_bound;

impl Hysteria2Adapter {
    pub(super) fn spawn_inbound_impl(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let proxy = proxy.clone();
        listeners.spawn(async move {
            let request = Hysteria2InboundListenerRequest::from_protocol_config(&inbound.protocol)?;
            run_hysteria2_listener_with_bound(&proxy, inbound, request, bound, shutdown_rx).await
        });
    }
}
