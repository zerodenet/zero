use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

impl Hysteria2Adapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        _proxy: &crate::runtime::Proxy,
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
        match super::connector::connect_tcp(session, server, *port, password, *client_fingerprint)
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                *tag, *server, *port, upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_hysteria2",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }
}
