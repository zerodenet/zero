use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::mixed::MixedAdapter;
use crate::adapters::socks5::inbound::socks5_acceptor_from_users;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

mod listener;

impl MixedAdapter {
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
            let InboundProtocolConfig::Mixed { socks5_users } = &inbound.protocol else {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "mixed adapter received non-mixed inbound config",
                )));
            };
            listener::run_mixed_listener_with_bound(
                &p,
                listener::MixedInboundRequest {
                    inbound_tag: inbound.tag,
                    socks5_acceptor: socks5_acceptor_from_users(socks5_users),
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
