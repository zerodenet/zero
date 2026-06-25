use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

impl Socks5Adapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::socks5::connect_tcp(
            proxy,
            session,
            server,
            *port,
            username.zip(*password),
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Socks5 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_socks5",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Socks5 {
            username, password, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::socks5::apply_tcp_hop(proxy, stream, session, username.zip(*password))
            .await
    }
}
