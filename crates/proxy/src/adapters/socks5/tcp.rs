use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure, TcpRelayStream};

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
        match connect_tcp(proxy, session, server, *port, *username, *password).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                *tag, *server, *port, upstream,
            )),
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
        apply_tcp_hop(proxy, stream, session, *username, *password).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let mut upstream = MeteredStream::new(upstream);

    let profile = socks5::Socks5TcpOutboundProfile::from_config_parts(username, password);
    profile.establish_tcp_tunnel(&mut upstream, session).await?;
    proxy.record_session_outbound_traffic(session.id, upstream.drain_traffic());

    Ok(upstream.into_inner().into())
}

async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let profile = socks5::Socks5TcpOutboundProfile::from_config_parts(username, password);
    profile
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(stream)
}
