use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

impl TrojanAdapter {
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
            let (profile, tls_cfg) = match &inbound.protocol {
                InboundProtocolConfig::Trojan { password, tls, .. } => {
                    let tls_cfg = tls.clone().ok_or_else(|| {
                        EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "trojan requires TLS",
                        ))
                    })?;
                    (
                        trojan::TrojanInboundProfile::from_config_password(password.as_str()),
                        tls_cfg,
                    )
                }
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "trojan adapter received non-trojan inbound config",
                    )));
                }
            };
            let tls_acceptor =
                crate::transport::build_tls_acceptor(&tls_cfg, p.config.source_dir())?;
            crate::inbound::run_trojan_listener_with_bound(
                &p,
                crate::inbound::trojan::TrojanInboundRequest {
                    inbound,
                    profile,
                    tls_acceptor,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
