use socks5::Socks5InboundTcpAcceptor;
use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

#[derive(Clone)]
pub(crate) struct Socks5InboundListenerRequest {
    pub(crate) acceptor: Socks5InboundTcpAcceptor,
}

pub(crate) fn socks5_acceptor_from_users(
    users: &[zero_config::Socks5UserConfig],
) -> Socks5InboundTcpAcceptor {
    socks5::Socks5InboundTcpAcceptor::from_config_users(users.iter().map(|user| {
        (
            user.username.as_str(),
            user.password.as_str(),
            user.principal_key.as_deref(),
            user.up_bps,
            user.down_bps,
        )
    }))
}

impl Socks5InboundListenerRequest {
    pub(crate) fn from_protocol_config(
        protocol: &InboundProtocolConfig,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Socks5 { users } => Ok(Self {
                acceptor: socks5_acceptor_from_users(users),
            }),
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "socks5 inbound request received non-socks5 inbound config",
            ))),
        }
    }
}
