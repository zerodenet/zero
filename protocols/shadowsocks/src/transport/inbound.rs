use core::future::Future;

use zero_core::Session;
use zero_traits::AsyncSocket;
use zero_transport::RuntimeError;

use super::{
    OwnedShadowsocksInboundBindings, OwnedShadowsocksInboundProfile,
    OwnedShadowsocksInboundTcpAcceptor,
};

pub fn inbound_profile_from_cipher_password(
    cipher: &str,
    password: &str,
) -> Result<OwnedShadowsocksInboundProfile, RuntimeError> {
    crate::inbound_profile_from_config_cipher_password(cipher, password)
        .map(OwnedShadowsocksInboundProfile::new)
        .map_err(|error| {
            RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid shadowsocks inbound profile: {error}"),
            ))
        })
}

impl OwnedShadowsocksInboundProfile {
    fn new(protocol: crate::ShadowsocksInboundProfile) -> Self {
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
    fn new(protocol: crate::ShadowsocksInboundTcpAcceptor) -> Self {
        Self { protocol }
    }

    pub async fn accept_and_dispatch_stream<S, H, HFut, E>(
        &self,
        stream: S,
        handoff: H,
    ) -> Result<(), E>
    where
        S: AsyncSocket,
        H: FnOnce(Session, crate::ShadowsocksAeadStream<S>) -> HFut,
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
        crate::udp::ShadowsocksInboundUdpRelay,
    ) {
        (self.acceptor, self.udp_relay)
    }
}
