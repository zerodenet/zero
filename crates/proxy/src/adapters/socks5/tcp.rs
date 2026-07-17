use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::{claim_socket_tcp_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::SocketTcpHandshake;

#[async_trait::async_trait]
impl SocketTcpHandshake for ::socks5::transport::Socks5TransportLeaf {
    fn tag(&self) -> &str {
        self.tag()
    }

    fn server(&self) -> &str {
        self.server()
    }

    fn port(&self) -> u16 {
        self.port()
    }

    fn connect_stage(&self) -> &'static str {
        "connect_upstream_socks5"
    }

    async fn open_tcp_stream(
        &self,
        services: crate::protocol_registry::TcpRuntimeServices,
        session: &zero_core::Session,
    ) -> Result<
        (
            crate::transport::TcpRelayStream,
            zero_transport::StreamTraffic,
        ),
        zero_transport::RuntimeError,
    > {
        ::socks5::transport::Socks5TransportLeaf::open_tcp_stream(
            self,
            session,
            move |server, port| {
                let server = server.to_owned();
                async move { services.connect_upstream_owned(server, port).await }
            },
        )
        .await
    }

    async fn open_tcp_relay_hop(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &zero_core::Session,
    ) -> Result<crate::transport::TcpRelayStream, zero_transport::RuntimeError> {
        ::socks5::transport::Socks5TransportLeaf::open_tcp_relay_hop(self, stream, session).await
    }
}

impl Socks5Adapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ::socks5::transport::Socks5TransportLeaf,
    ) -> Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a> {
        claim_socket_tcp_leaf(leaf)
    }
}
