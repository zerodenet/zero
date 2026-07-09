mod request;
mod transport;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use request::MieruInboundListenerRequest;
pub(crate) use transport::run_mieru_listener_with_bound;

impl MieruAdapter {
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
            let request = MieruInboundListenerRequest::from_protocol_config(&inbound.protocol)?;
            run_mieru_listener_with_bound(&proxy, inbound, request, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}
