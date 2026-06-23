use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

impl Hysteria2Adapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            insecure: _,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::hysteria2::connect_tcp(
            proxy,
            session,
            server,
            *port,
            password,
            *client_fingerprint,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Hysteria2 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_hysteria2",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }
}
