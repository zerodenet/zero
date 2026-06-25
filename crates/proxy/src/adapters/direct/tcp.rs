use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::direct::DirectAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

impl DirectAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match proxy
            .protocols
            .direct_connector()
            .connect(session, proxy.resolver.as_ref())
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Direct {
                tag: (*tag).unwrap_or("direct").to_string(),
                upstream: upstream.into(),
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_direct",
                error: error.into(),
                upstream_endpoint: None,
            }),
        }
    }
}
