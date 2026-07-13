use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::direct::DirectAdapter;
use crate::runtime::inbound_operation::PreparedInboundListenerOperation;

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
        let target = target.map(|value| {
            if let Ok(address) = value.parse::<std::net::Ipv4Addr>() {
                zero_core::Address::Ipv4(address.octets())
            } else if let Ok(address) = value.parse::<std::net::Ipv6Addr>() {
                zero_core::Address::Ipv6(address.octets())
            } else {
                zero_core::Address::Domain(value)
            }
        });
        Ok(Box::new(crate::inbound::DirectInboundListenerOperation {
            inbound_tag: inbound.tag,
            target,
            port,
        }))
    }
}
