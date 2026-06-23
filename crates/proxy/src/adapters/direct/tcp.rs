use super::*;

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
