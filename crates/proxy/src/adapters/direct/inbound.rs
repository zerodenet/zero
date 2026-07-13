use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::direct::DirectAdapter;
use crate::runtime::inbound_operation::{
    InboundListenerOperation, PreparedInboundListenerOperation,
};

impl DirectAdapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: InboundConfig,
    ) -> Result<Box<dyn PreparedInboundListenerOperation>, EngineError> {
        let (target, port) = match &inbound.protocol {
            InboundProtocolConfig::Direct { target, port } => (target.clone(), *port),
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "direct adapter received non-direct inbound config",
                )));
            }
        };
        Ok(Box::new(InboundListenerOperation::new(
            move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                crate::inbound::run_direct_listener_with_bound(
                    &proxy,
                    crate::inbound::direct::DirectInboundRequest {
                        inbound,
                        target,
                        port,
                    },
                    bound.into_tcp(),
                    shutdown_rx,
                )
                .await
            },
        )))
    }
}
