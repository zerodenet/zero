use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

#[derive(Clone)]
pub(crate) struct MieruInboundListenerRequest {
    pub(crate) profile: mieru::inbound::MieruInboundProfile,
}

impl MieruInboundListenerRequest {
    pub(crate) fn from_protocol_config(
        protocol: &InboundProtocolConfig,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Mieru { users } => Ok(Self {
                profile: mieru::inbound::inbound_profile_from_config_users(
                    users
                        .iter()
                        .map(|user| (user.username.as_str(), user.password.as_str())),
                ),
            }),
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "mieru inbound request received non-mieru inbound config",
            ))),
        }
    }
}
