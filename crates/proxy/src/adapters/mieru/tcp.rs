use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

impl MieruAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match connect_tcp(proxy, session, server, *port, username, password).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_mieru",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Mieru {
            username, password, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        apply_tcp_hop(stream, session, username, password).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream = TcpRelayStream::new(socket);
    let mieru_stream = mieru::establish_tcp_tunnel(
        stream,
        &mieru::MieruTcpTunnelTarget::new(session, username, password),
    )
    .await
    .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru tcp tunnel: {e}"))))?;
    Ok(TcpRelayStream::new(mieru_stream))
}

async fn apply_tcp_hop(
    stream: TcpRelayStream,
    session: &Session,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let mieru_stream = mieru::establish_tcp_tunnel(
        stream,
        &mieru::MieruTcpTunnelTarget::new(session, username, password),
    )
    .await
    .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru tcp tunnel: {e}"))))?;
    Ok(TcpRelayStream::new(mieru_stream))
}
