use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

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
            crate::inbound::run_socks5_listener_with_bound(
                &p,
                crate::inbound::Socks5InboundRequest {
                    auth: socks5::password_auth_from_config_users(users.iter().map(|user| {
                        (
                            user.username.as_str(),
                            user.password.as_str(),
                            user.principal_key.as_deref(),
                            user.up_bps,
                            user.down_bps,
                        )
                    })),
                    inbound,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
