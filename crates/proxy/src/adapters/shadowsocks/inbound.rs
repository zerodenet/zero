use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

mod listener;

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
            let profile = match &inbound.protocol {
                InboundProtocolConfig::Shadowsocks {
                    password, cipher, ..
                } => shadowsocks::inbound_profile_from_config_cipher_password(
                    cipher.as_str(),
                    password.as_str(),
                )
                .map_err(|error| {
                    EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid shadowsocks inbound profile: {error}"),
                    ))
                })?,
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "shadowsocks adapter received non-shadowsocks inbound config",
                    )));
                }
            };
            let udp_session = profile.accept_udp_session_with_auth();
            listener::run_shadowsocks_listener_with_bound(
                &p,
                listener::ShadowsocksInboundRequest {
                    inbound,
                    profile,
                    udp_session,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
