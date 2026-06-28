use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_platform_tokio::TransportConnector;

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

fn vless_tcp_config(
    id: &str,
    flow: Option<&str>,
) -> Result<vless::VlessTcpConnectConfig, EngineError> {
    vless::VlessTcpConnectConfig::from_config(id, flow).map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })
}

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
        let config = vless_tcp_config(id, *flow).map_err(|error| TcpOutboundFailure {
            stage: "connect_upstream_vless",
            error,
            upstream_endpoint: Some(((*server).to_string(), *port)),
        })?;
        if *flow == Some("xtls-rprx-vision") {
            return self
                .mux_pool
                .open_stream(VlessMuxOpenRequest {
                    proxy,
                    session: Some(session),
                    server,
                    port: *port,
                    identity: vless::mux_pool::MuxIdentity::from_uuid(config.id()),
                    tls: *tls,
                    reality: *reality,
                    max_concurrency: mux_concurrency.unwrap_or(8),
                })
                .await
                .map(|upstream| EstablishedTcpOutbound::Vless {
                    tag: (*tag).to_string(),
                    server: (*server).to_string(),
                    port: *port,
                    upstream,
                })
                .map_err(|error| TcpOutboundFailure {
                    stage: "connect_upstream_vless",
                    error,
                    upstream_endpoint: Some(((*server).to_string(), *port)),
                });
        }
        match connect_tcp(VlessTcpConnect {
            proxy,
            session,
            server,
            port: *port,
            config,
            mux_concurrency: *mux_concurrency,
            mux_idle_timeout_secs: *mux_idle_timeout_secs,
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            quic: *quic,
            split_http: *split_http,
        })
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vless {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
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
        let config = vless_tcp_config(id, *flow)?;
        apply_tcp_hop(proxy, stream, session, config).await
    }
}

struct VlessTcpConnect<'a> {
    proxy: &'a Proxy,
    session: &'a Session,
    server: &'a str,
    port: u16,
    config: vless::VlessTcpConnectConfig,
    mux_concurrency: Option<u32>,
    mux_idle_timeout_secs: Option<u64>,
    tls: Option<&'a zero_config::ClientTlsConfig>,
    reality: Option<&'a zero_config::RealityConfig>,
    ws: Option<&'a zero_config::WebSocketConfig>,
    grpc: Option<&'a zero_config::GrpcConfig>,
    h2: Option<&'a zero_config::H2Config>,
    http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    quic: Option<&'a zero_config::QuicConfig>,
    split_http: Option<&'a zero_config::SplitHttpConfig>,
}

async fn connect_tcp(request: VlessTcpConnect<'_>) -> Result<TcpRelayStream, EngineError> {
    let VlessTcpConnect {
        proxy,
        session,
        server,
        port,
        config,
        mux_concurrency,
        mux_idle_timeout_secs,
        tls,
        reality,
        ws,
        grpc,
        h2,
        http_upgrade,
        quic,
        split_http,
    } = request;

    let _ = mux_concurrency;
    let _ = mux_idle_timeout_secs;

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

    let connector =
        crate::transport::VlessTransportConnector::new(crate::transport::VlessTransportOptions {
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            source_dir: proxy.config.source_dir(),
        });
    let stream = connector.connect(socket, server, port).await?;

    let is_reality = reality.is_some();
    let mut metered = crate::transport::MeteredStream::new(stream);

    if is_reality {
        use zero_traits::DeferredTcpTunnelProtocol;
        vless::VlessOutbound
            .send_deferred_tcp_tunnel_request(&mut metered, &config.flow_tcp_target(session))
            .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(TcpRelayStream::new(
            vless::DeferredVlessResponseStream::new(metered.into_inner()),
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
    if config.flow().is_some() {
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
