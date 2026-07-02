use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_platform_tokio::TransportConnector;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

impl VlessAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
            id,
            flow,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config = vless::tcp_connect_config_from_config(id, *flow).map_err(|error| {
            TcpOutboundFailure {
                stage: "connect_upstream_vless",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    error,
                )),
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }
        })?;
        if config.should_open_mux_pool_for_tcp() {
            return crate::adapters::vless::mux_pool::open_stream(
                &self.mux_pool,
                VlessMuxOpenRequest {
                    proxy,
                    session: Some(session),
                    server,
                    port: *port,
                    identity: config.mux_pool_identity(),
                    tls: *tls,
                    reality: *reality,
                    max_concurrency: mux_concurrency.unwrap_or(8),
                },
            )
            .await
            .map(|upstream| EstablishedTcpOutbound::proxied(*tag, *server, *port, upstream))
            .map_err(|error| TcpOutboundFailure {
                stage: "connect_upstream_vless",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            });
        }
        let _ = mux_concurrency;
        let _ = mux_idle_timeout_secs;
        let transport = crate::transport::VlessTransportOptions {
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            split_http: *split_http,
            source_dir: proxy.config.source_dir(),
        };
        match connect_tcp(proxy, session, server, *port, config, *quic, transport).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                *tag, *server, *port, upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vless",
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
        let ResolvedLeafOutbound::Vless { id, flow, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config = vless::tcp_connect_config_from_config(id, *flow).map_err(|error| {
            EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
        })?;
        apply_tcp_hop(proxy, stream, session, config).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    config: vless::VlessTcpConnectConfig,
    quic: Option<&zero_config::QuicConfig>,
    transport: crate::transport::VlessTransportOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    if let Some(quic) = quic {
        let server_name = quic.server_name.as_deref().unwrap_or(server);
        let quic_stream = crate::transport::connect_quic(server_name, port, quic.insecure).await?;
        return Ok(TcpRelayStream::new(quic_stream));
    }

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let connector = crate::transport::VlessTransportConnector::new(transport);
    let stream = connector.connect(socket, server, port).await?;

    let is_reality = transport.reality.is_some();
    let mut metered = crate::transport::MeteredStream::new(stream);

    if is_reality {
        use zero_traits::DeferredTcpTunnelProtocol;
        vless::VlessOutbound
            .send_deferred_tcp_tunnel_request(&mut metered, &config.flow_tcp_target(session))
            .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(TcpRelayStream::new(
            config.wrap_deferred_response_stream(metered.into_inner()),
        ))
    } else {
        use zero_traits::TcpTunnelProtocol;
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessFlowTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut metered,
            &config.flow_tcp_target(session),
        )
        .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(metered.into_inner())
    }
}

async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    config: vless::VlessTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    use zero_traits::TcpTunnelProtocol;
    if config.has_flow() {
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessFlowTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut stream,
            &config.flow_tcp_target(session),
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    } else {
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut stream,
            &config.tcp_target(session),
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    }
    Ok(stream)
}
