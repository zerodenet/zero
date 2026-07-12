mod transport;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use transport::{handle_socks5_connection, run_socks5_listener_with_bound};

impl Socks5Adapter {
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
            let acceptor = zero_transport::socks5_transport::inbound_acceptor_from_protocol(
                &inbound.protocol,
            )?;
            run_socks5_listener_with_bound(&proxy, inbound, acceptor, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}
