use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::{claim_socket_tcp_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::SocketTcpHandshake;

#[async_trait::async_trait]
impl SocketTcpHandshake for ::mieru::transport::MieruTransportLeaf {
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
        "connect_upstream_mieru"
    }

    async fn open_tcp_stream(
        &self,
        services: crate::protocol_registry::UpstreamConnectServices,
        session: &zero_core::Session,
    ) -> Result<
        (
            crate::transport::TcpRelayStream,
            zero_transport::StreamTraffic,
        ),
        zero_transport::RuntimeError,
    > {
        let stream = ::mieru::transport::MieruTransportLeaf::open_tcp_stream(
            self,
            session,
            move |server, port| {
                let services = services.clone();
                let server = server.to_owned();
                async move { services.connect_upstream_owned(server, port).await }
            },
        )
        .await?;
        Ok((stream, zero_transport::StreamTraffic::default()))
    }

    async fn open_tcp_relay_hop(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &zero_core::Session,
    ) -> Result<crate::transport::TcpRelayStream, zero_transport::RuntimeError> {
        ::mieru::transport::MieruTransportLeaf::open_tcp_relay_hop(self, stream, session).await
    }
}

impl MieruAdapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ::mieru::transport::MieruTransportLeaf,
    ) -> Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a> {
        claim_socket_tcp_leaf(leaf)
    }
}
