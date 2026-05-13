use zero_config::{ClientTlsConfig, GrpcConfig, H2Config, QuicConfig, RealityConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
#[cfg(feature = "outbound-socks5")]
use crate::transport::MeteredStream;
use crate::transport::TcpRelayStream;

pub(crate) struct VlessUpstream<'a> {
    pub server: &'a str,
    pub port: u16,
    pub id: &'a str,
    pub flow: Option<&'a str>,
    pub mux_concurrency: Option<u32>,
    pub mux_idle_timeout_secs: Option<u64>,
    pub tls: Option<&'a ClientTlsConfig>,
    pub reality: Option<&'a RealityConfig>,
    pub ws: Option<&'a zero_config::WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
    pub http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub quic: Option<&'a QuicConfig>,
}

impl Proxy {
    #[cfg(feature = "outbound-socks5")]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, &self.resolver)
            .await?;
        let mut upstream = MeteredStream::new(upstream);

        self.protocols
            .socks5_outbound
            .establish_tunnel_with_auth(
                &mut upstream,
                session,
                auth.map(
                    |(username, password)| zero_protocol_socks5::Socks5OutboundAuth {
                        username,
                        password,
                    },
                ),
            )
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner().into())
    }

    #[cfg(not(feature = "outbound-socks5"))]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _auth: Option<(&str, &str)>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "socks5-upstream".to_owned(),
            protocol: "socks5",
            feature: "outbound-socks5",
        })
    }

    #[cfg(feature = "outbound-vless")]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        session: &Session,
        upstream: VlessUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let id = zero_protocol_vless::parse_uuid(upstream.id)?;

        // If MUX flow is configured, use connection pool
        if upstream.flow == Some("xtls-rprx-vision") {
            return self
                .mux_pool
                .open_stream(
                    self,
                    session,
                    upstream.server.to_owned(),
                    upstream.port,
                    &id,
                    upstream.tls,
                    upstream.reality,
                    upstream.mux_concurrency.unwrap_or(8),
                    upstream.mux_idle_timeout_secs.unwrap_or(300),
                )
                .await;
        }

        // QUIC uses UDP — handle before TCP connect entirely
        if let Some(quic) = upstream.quic {
            let server_name = quic.server_name.as_deref().unwrap_or(upstream.server);
            let quic_stream =
                crate::transport::connect_quic(server_name, upstream.port, quic.insecure).await?;
            return Ok(TcpRelayStream::new(quic_stream));
        }

        let socket = self
            .protocols
            .direct_outbound
            .connect_host(upstream.server, upstream.port, &self.resolver)
            .await?;

        let connector = zero_protocol_vless::VlessTransportConnector::new(
            upstream.tls,
            upstream.reality,
            upstream.ws,
            upstream.grpc,
            upstream.h2,
            upstream.http_upgrade,
            self.config.source_dir(),
        );
        let stream = connector
            .connect(socket, upstream.server, upstream.port)
            .await?;

        let flow = upstream.flow;
        let is_reality = upstream.reality.is_some();
        let mut metered = crate::transport::MeteredStream::new(stream);

        if is_reality {
            self.protocols
                .vless_outbound
                .send_tcp_request_with_flow(&mut metered, session, &id, flow)
                .await?;
            self.record_session_outbound_traffic(session.id, metered.drain_traffic());

            Ok(TcpRelayStream::new(
                zero_protocol_vless::DeferredVlessResponseStream::new(metered.into_inner()),
            ))
        } else {
            self.protocols
                .vless_outbound
                .establish_tcp_tunnel_with_flow(&mut metered, session, &id, flow)
                .await?;
            self.record_session_outbound_traffic(session.id, metered.drain_traffic());

            Ok(metered.into_inner())
        }
    }

    #[cfg(not(feature = "outbound-vless"))]
    pub(crate) async fn connect_via_vless_upstream(
        &self,
        _session: &Session,
        upstream: VlessUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = (
            upstream.server,
            upstream.port,
            upstream.id,
            upstream.mux_concurrency,
            upstream.mux_idle_timeout_secs,
            upstream.tls,
            upstream.reality,
            upstream.ws,
        );

        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "vless-upstream".to_owned(),
            protocol: "vless",
            feature: "outbound-vless",
        })
    }

    #[cfg(feature = "outbound-hysteria2")]
    pub(crate) async fn connect_via_hysteria2_upstream(
        &self,
        _session: &Session,
        server: &str,
        port: u16,
    ) -> Result<TcpRelayStream, EngineError> {
        let quic_stream =
            crate::transport::connect_quic(server, port, true).await?;
        Ok(TcpRelayStream::new(quic_stream))
    }

    #[cfg(not(feature = "outbound-hysteria2"))]
    pub(crate) async fn connect_via_hysteria2_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "hysteria2-upstream".to_owned(),
            protocol: "hysteria2",
            feature: "outbound-hysteria2",
        })
    }
}
