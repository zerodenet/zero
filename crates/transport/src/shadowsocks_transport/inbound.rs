use core::future::Future;

use zero_config::InboundProtocolConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use super::{
    OwnedShadowsocksInboundBindings, OwnedShadowsocksInboundProfile,
    OwnedShadowsocksInboundTcpAcceptor,
};

pub fn inbound_profile_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedShadowsocksInboundProfile, EngineError> {
    match protocol {
        InboundProtocolConfig::Shadowsocks {
            password, cipher, ..
        } => shadowsocks::inbound_profile_from_config_cipher_password(
            cipher.as_str(),
            password.as_str(),
        )
        .map(OwnedShadowsocksInboundProfile::new)
        .map_err(|error| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid shadowsocks inbound profile: {error}"),
            ))
        }),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shadowsocks inbound profile received non-shadowsocks inbound config",
        ))),
    }
}

impl OwnedShadowsocksInboundProfile {
    fn new(protocol: shadowsocks::ShadowsocksInboundProfile) -> Self {
        Self { protocol }
    }

    pub fn into_listener_bindings(self) -> OwnedShadowsocksInboundBindings {
        let (acceptor, udp_relay) = self.protocol.into_listener_bindings();
        OwnedShadowsocksInboundBindings {
            acceptor: OwnedShadowsocksInboundTcpAcceptor::new(acceptor),
            udp_relay,
        }
    }
}

impl OwnedShadowsocksInboundTcpAcceptor {
    fn new(protocol: shadowsocks::ShadowsocksInboundTcpAcceptor) -> Self {
        Self { protocol }
    }

    pub async fn accept_and_dispatch_stream<S, H, HFut, E>(
        &self,
        stream: S,
        handoff: H,
    ) -> Result<(), E>
    where
        S: AsyncSocket,
        H: FnOnce(Session, shadowsocks::ShadowsocksAeadStream<S>) -> HFut,
        HFut: Future<Output = Result<(), E>>,
        E: From<zero_core::Error>,
    {
        self.protocol
            .accept_and_dispatch_stream(stream, handoff)
            .await
    }
}

impl OwnedShadowsocksInboundBindings {
    pub fn into_parts(
        self,
    ) -> (
        OwnedShadowsocksInboundTcpAcceptor,
        shadowsocks::udp::ShadowsocksInboundUdpRelay,
    ) {
        (self.acceptor, self.udp_relay)
    }
}
