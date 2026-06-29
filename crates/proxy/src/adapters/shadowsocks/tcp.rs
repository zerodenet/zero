use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure, TcpRelayStream};

fn invalid_shadowsocks_tcp_config(
    error: impl std::fmt::Display,
    stage: &'static str,
) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

fn invalid_shadowsocks_tcp_failure(
    error: impl std::fmt::Display,
    upstream_endpoint: Option<(String, u16)>,
) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "connect_upstream_shadowsocks",
        error: invalid_shadowsocks_tcp_config(error, "invalid shadowsocks tcp config"),
        upstream_endpoint,
    }
}

impl ShadowsocksAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config =
            shadowsocks::tcp_connect_config_from_config(cipher, password).map_err(|error| {
                invalid_shadowsocks_tcp_failure(error, Some(((*server).to_string(), *port)))
            })?;
        match connect_tcp(proxy, session, server, *port, config).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                *tag, *server, *port, upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_shadowsocks",
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
        let ResolvedLeafOutbound::Shadowsocks {
            password, cipher, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config =
            shadowsocks::tcp_connect_config_from_config(cipher, password).map_err(|error| {
                invalid_shadowsocks_tcp_config(error, "invalid shadowsocks tcp relay config")
            })?;
        apply_tcp_hop(stream, session, config).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    config: shadowsocks::ShadowsocksTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let mut metered = MeteredStream::new(upstream);
    let ss_session = config.establish_tcp_session(&mut metered, session).await?;
    proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());
    let upstream = TcpRelayStream::from(metered.into_inner());
    Ok(TcpRelayStream::new(
        config.wrap_outbound_stream(upstream, ss_session),
    ))
}

async fn apply_tcp_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    config: shadowsocks::ShadowsocksTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    let ss_session = config
        .establish_tcp_session(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(TcpRelayStream::new(
        config.wrap_outbound_stream(stream, ss_session),
    ))
}
