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
            crate::inbound::run_socks5_listener_with_bound(
                &p,
                crate::inbound::Socks5InboundRequest {
                    auth: socks5_auth_from_config(&inbound.protocol)?,
                    inbound,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}

fn socks5_auth_from_config(
    config: &InboundProtocolConfig,
) -> Result<socks5::ConfiguredSocks5PasswordAuth, EngineError> {
    let InboundProtocolConfig::Socks5 { users } = config else {
        return Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "socks5 adapter received non-socks5 inbound config",
        )));
    };
    Ok(socks5::ConfiguredSocks5PasswordAuth::from_config_users(
        users.iter().map(|user| {
            (
                user.username.clone(),
                user.password.clone(),
                user.principal_key.clone(),
                user.up_bps,
                user.down_bps,
            )
        }),
    ))
}
