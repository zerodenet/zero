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
            crate::inbound::run_mixed_listener_with_bound(
                &p,
                crate::inbound::MixedInboundRequest {
                    socks5_auth: socks5_auth_from_config(&inbound.protocol)?,
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
    let InboundProtocolConfig::Mixed { socks5_users } = config else {
        return Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "mixed adapter received non-mixed inbound config",
        )));
    };
    Ok(socks5::ConfiguredSocks5PasswordAuth::from_config_parts(
        socks5_users.iter().map(|user| {
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
