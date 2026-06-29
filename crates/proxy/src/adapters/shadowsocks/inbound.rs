use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

impl ShadowsocksAdapter {
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
            let (password, cipher_name) = match &inbound.protocol {
                InboundProtocolConfig::Shadowsocks {
                    password, cipher, ..
                } => (password.clone(), cipher.clone()),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "shadowsocks adapter received non-shadowsocks inbound config",
                    )));
                }
            };
            let profile = shadowsocks::ShadowsocksInboundProfile::from_config_cipher_password(
                &cipher_name,
                &password,
            )
            .map_err(|error| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid shadowsocks inbound profile: {error}"),
                ))
            })?;
            crate::inbound::run_shadowsocks_listener_with_bound(
                &p,
                crate::inbound::shadowsocks::ShadowsocksInboundRequest { inbound, profile },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
