mod request;
mod transport;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use request::ShadowsocksInboundListenerRequest;
pub(crate) use transport::run_shadowsocks_listener_with_bound;

impl ShadowsocksAdapter {
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
            let request =
                ShadowsocksInboundListenerRequest::from_protocol_config(&inbound.protocol)?;
            run_shadowsocks_listener_with_bound(
                &proxy,
                inbound,
                request,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
