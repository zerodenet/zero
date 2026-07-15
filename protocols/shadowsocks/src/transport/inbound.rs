use core::future::Future;

use zero_core::Session;
use zero_traits::AsyncSocket;
use zero_transport::RuntimeError;

use super::model::{
    ShadowsocksInboundBindings, ShadowsocksInboundProfile, ShadowsocksInboundTcpAcceptor,
};
use super::ShadowsocksInboundOptionsRef;

impl ShadowsocksInboundProfile {
    fn new(protocol: crate::ShadowsocksInboundProfile) -> Self {
        Self { protocol }
    }

    fn into_listener_bindings(self) -> ShadowsocksInboundBindings {
        let (acceptor, udp_relay) = self.protocol.into_listener_bindings();
        ShadowsocksInboundBindings {
            acceptor: ShadowsocksInboundTcpAcceptor::new(acceptor),
            udp_relay,
        }
    }
}

impl ShadowsocksInboundTcpAcceptor {
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

impl ShadowsocksInboundBindings {
    pub fn from_options_refs(
        options: ShadowsocksInboundOptionsRef<'_>,
    ) -> Result<Self, RuntimeError> {
        crate::inbound_profile_from_config_cipher_password(options.cipher, options.password)
            .map(ShadowsocksInboundProfile::new)
            .map(ShadowsocksInboundProfile::into_listener_bindings)
            .map_err(|error| {
                RuntimeError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid shadowsocks inbound profile: {error}"),
                ))
            })
    }

    pub fn into_parts(
        self,
    ) -> (
        ShadowsocksInboundTcpAcceptor,
        crate::udp::ShadowsocksInboundUdpRelay,
    ) {
        (self.acceptor, self.udp_relay)
    }
}
