use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;
use crate::transport::QuicInbound;

impl Hysteria2Adapter {
    pub(super) async fn bind_inbound_impl(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        if let InboundProtocolConfig::Hysteria2 {
            cert_path,
            key_path,
            ..
        } = &inbound.protocol
        {
            let cert = cert_path
                .clone()
                .unwrap_or_else(|| "certs/fullchain.pem".to_string());
            let key = key_path
                .clone()
                .unwrap_or_else(|| "certs/privkey.pem".to_string());
            let endpoint = QuicInbound::bind(&listen, &cert, &key, source_dir).await?;
            Ok(BoundInbound::Quic(endpoint))
        } else {
            unreachable!("hysteria2 adapter only handles Hysteria2 config")
        }
    }

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
                InboundProtocolConfig::Hysteria2 { password, .. } => {
                    hysteria2::inbound_profile_from_config_password(password.as_str())
                }
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "hysteria2 adapter received non-hysteria2 inbound config",
                    )));
                }
            };
            crate::inbound::run_hysteria2_listener_with_bound(
                &p,
                crate::inbound::hysteria2::Hysteria2InboundRequest { inbound, profile },
                bound,
                shutdown_rx,
            )
            .await
        });
    }
}
