use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::direct::DirectAdapter;
use crate::protocol_adapter::BoundInbound;
use crate::runtime::Proxy;

impl DirectAdapter {
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
            let (target, port) = match &inbound.protocol {
                InboundProtocolConfig::Direct { target, port } => (target.clone(), *port),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "direct adapter received non-direct inbound config",
                    )));
                }
            };
            crate::inbound::run_direct_listener_with_bound(
                &p,
                crate::inbound::direct::DirectInboundRequest {
                    inbound,
                    target,
                    port,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
