use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::http_connect::HttpConnectAdapter;
use crate::protocol_adapter::BoundInbound;
use crate::runtime::Proxy;

impl HttpConnectAdapter {
    pub(super) fn spawn_inbound_impl(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let p = proxy.clone();
        listeners.spawn(async move {
            crate::inbound::run_http_connect_listener_with_bound(
                &p,
                inbound,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
