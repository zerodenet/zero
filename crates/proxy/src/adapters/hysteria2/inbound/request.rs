use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

#[derive(Clone)]
pub(crate) struct Hysteria2InboundListenerRequest {
    pub(crate) profile: hysteria2::inbound::Hysteria2InboundProfile,
}

impl Hysteria2InboundListenerRequest {
    pub(crate) fn from_protocol_config(
        protocol: &InboundProtocolConfig,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Hysteria2 { password, .. } => Ok(Self {
                profile: hysteria2::inbound::inbound_profile_from_config_password(
                    password.as_str(),
                ),
            }),
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "hysteria2 inbound request received non-hysteria2 inbound config",
            ))),
        }
    }
}
