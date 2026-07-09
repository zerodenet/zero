use shadowsocks::ShadowsocksInboundProfile;
use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

pub(crate) struct ShadowsocksInboundListenerRequest {
    pub(crate) profile: ShadowsocksInboundProfile,
    pub(crate) udp_session: shadowsocks::udp::ShadowsocksInboundAcceptedUdpSession,
}

impl ShadowsocksInboundListenerRequest {
    pub(crate) fn from_protocol_config(
        protocol: &InboundProtocolConfig,
    ) -> Result<Self, EngineError> {
        let profile = match protocol {
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
                    "shadowsocks inbound request received non-shadowsocks inbound config",
                )));
            }
        };

        let udp_session = profile.accept_udp_session_with_auth();
        Ok(Self {
            profile,
            udp_session,
        })
    }
}
