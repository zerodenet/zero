use zero_config::{InboundConfig, InboundProtocolConfig, Socks5UserConfig};
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(in crate::adapters) mod listener;

pub(in crate::adapters) fn config_user_refs(
    users: &[Socks5UserConfig],
) -> impl Iterator<Item = (&str, &str, Option<&str>, Option<u64>, Option<u64>)> {
    users.iter().map(|user| {
        (
            user.username.as_str(),
            user.password.as_str(),
            user.principal_key.as_deref(),
            user.up_bps,
            user.down_bps,
        )
    })
}

impl Socks5Adapter {
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
            let InboundProtocolConfig::Socks5 { users } = &inbound.protocol else {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "socks5 adapter received non-socks5 inbound config",
                )));
            };
            listener::run_socks5_listener_with_bound(
                &p,
                listener::Socks5InboundRequest {
                    inbound_tag: inbound.tag,
                    acceptor: socks5::Socks5InboundTcpAcceptor::from_config_users(
                        config_user_refs(users),
                    ),
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
