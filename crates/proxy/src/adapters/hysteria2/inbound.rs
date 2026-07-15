//! Hysteria2 inbound profile preparation.

use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

use crate::runtime::inbound_operation::AuthenticatedQuicInboundListenerOperation;

impl crate::adapters::hysteria2::Hysteria2Adapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let profile = match &inbound.protocol {
            InboundProtocolConfig::Hysteria2 { password, .. } => {
                ::hysteria2::transport::inbound_profile_from_password(password)
            }
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "hysteria2 inbound listener received non-hysteria2 inbound config",
                )));
            }
        };
        Ok(Box::new(AuthenticatedQuicInboundListenerOperation {
            protocol_name: "hysteria2",
            profile,
        }))
    }
}
