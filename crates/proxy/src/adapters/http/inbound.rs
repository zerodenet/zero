mod listener;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::http::HttpConnectAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use listener::{run_http_listener_with_bound, HttpConnectInboundHandler};

impl HttpConnectAdapter {
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
            run_http_listener_with_bound(&proxy, inbound, bound.into_tcp(), shutdown_rx).await
        });
    }
}
