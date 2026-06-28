use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::TcpSessionProtocol;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure, TcpRelayStream};

fn tcp_config(
    cipher: &str,
    password: &str,
    stage: &'static str,
) -> Result<shadowsocks::ShadowsocksTcpConnectConfig, EngineError> {
    shadowsocks::ShadowsocksTcpConnectConfig::from_config(cipher, password).map_err(|error| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{stage}: {error}"),
        ))
    })
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
            tcp_config(cipher, password, "invalid shadowsocks tcp config").map_err(|error| {
                TcpOutboundFailure {
                    stage: "connect_upstream_shadowsocks",
                    error,
                    upstream_endpoint: Some(((*server).to_string(), *port)),
                }
            })?;
        match connect_tcp(proxy, session, server, *port, config).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Shadowsocks {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
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
        let config = tcp_config(cipher, password, "invalid shadowsocks tcp relay config")?;
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
    let ss_session = <shadowsocks::ShadowsocksOutbound as TcpSessionProtocol<
        shadowsocks::ShadowsocksTcpTarget,
    >>::establish_tcp_session(
        &shadowsocks::ShadowsocksOutbound,
        &mut metered,
        &config.tcp_target(session),
    )
    .await?;
    proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());
    Ok(wrap_outbound_stream(
        metered.into_inner().into(),
        ss_session,
        config.password_bytes().to_vec(),
    ))
}

fn wrap_outbound_stream(
    upstream: TcpRelayStream,
    ss_session: shadowsocks::ShadowsocksOutboundSession,
    password: Vec<u8>,
) -> TcpRelayStream {
    TcpRelayStream::new(shadowsocks::ShadowsocksAeadStream::outbound(
        upstream, ss_session, password,
    ))
}

async fn apply_tcp_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    config: shadowsocks::ShadowsocksTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    let ss_session = <shadowsocks::ShadowsocksOutbound as TcpSessionProtocol<
        shadowsocks::ShadowsocksTcpTarget,
    >>::establish_tcp_session(
        &shadowsocks::ShadowsocksOutbound,
        &mut stream,
        &config.tcp_target(session),
    )
    .await
    .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(wrap_outbound_stream(
        stream,
        ss_session,
        config.password_bytes().to_vec(),
    ))
}
