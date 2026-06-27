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
            let (password, tls) = match &inbound.protocol {
                InboundProtocolConfig::Trojan { password, tls, .. } => {
                    (password.clone(), tls.clone())
                }
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "trojan adapter received non-trojan inbound config",
                    )));
                }
            };
            crate::inbound::run_trojan_listener_with_bound(
                &p,
                crate::inbound::trojan::TrojanInboundRequest {
                    inbound,
                    password,
                    tls,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
