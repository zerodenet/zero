use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::mixed::MixedAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

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
            crate::inbound::run_mixed_listener_with_bound(
                &p,
                crate::inbound::MixedInboundRequest {
                    socks5_auth: socks5::password_auth_from_config_users(socks5_users.iter().map(
                        |user| {
                            (
                                user.username.as_str(),
                                user.password.as_str(),
                                user.principal_key.as_deref(),
                                user.up_bps,
                                user.down_bps,
                            )
                        },
                    )),
                    inbound,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
