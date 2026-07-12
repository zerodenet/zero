use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};
use zero_transport::mieru_transport::MieruTransportLeaf;

impl MieruAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let Some(leaf) = MieruTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match connect_tcp(proxy, session, &leaf).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                leaf.tag().to_owned(),
                leaf.server().to_owned(),
                leaf.port(),
                upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_mieru",
                error,
                upstream_endpoint: Some((leaf.server().to_string(), leaf.port())),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let Some(leaf) = MieruTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        apply_tcp_hop(stream, session, &leaf).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    leaf: &MieruTransportLeaf<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let connector = proxy.protocols.direct_connector();
    let resolver = proxy.resolver.clone();
    let server = leaf.server().to_owned();
    let port = leaf.port();
    leaf.open_tcp_stream(session, move |_, _| {
        let server = server.clone();
        let resolver = resolver.clone();
        async move {
            connector
                .connect_host(&server, port, resolver.as_ref())
                .await
        }
    })
    .await
}

async fn apply_tcp_hop(
    stream: TcpRelayStream,
    session: &Session,
    leaf: &MieruTransportLeaf<'_>,
) -> Result<TcpRelayStream, EngineError> {
    leaf.open_tcp_relay_hop(stream, session).await
}
