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
    pub split_http: Option<&'a zero_config::SplitHttpConfig>,
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
            .connect_host(server, port, self.resolver.as_ref())
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
            .connect_host(upstream.server, upstream.port, self.resolver.as_ref())
            .await?;

        let connector = crate::transport::VlessTransportConnector::new(
            upstream.tls,
            upstream.reality,
            upstream.ws,
            upstream.grpc,
            upstream.h2,
            upstream.http_upgrade,
            upstream.split_http,
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
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(server, port, password);
        let stream = connector.connect(session).await?;
        Ok(TcpRelayStream::new(stream))
    }

    #[cfg(not(feature = "outbound-hysteria2"))]
    pub(crate) async fn connect_via_hysteria2_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _password: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "hysteria2-upstream".to_owned(),
            protocol: "hysteria2",
            feature: "outbound-hysteria2",
        })
    }

    #[cfg(feature = "outbound-shadowsocks")]
    pub(crate) async fn connect_via_shadowsocks_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        cipher: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        use zero_protocol_shadowsocks::CipherKind;
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, self.resolver.as_ref())
            .await?;
        let mut metered = crate::transport::MeteredStream::new(upstream);
        let cipher_kind = CipherKind::from_str(cipher).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown shadowsocks cipher: {cipher}"),
            ))
        })?;
        self.protocols
            .shadowsocks_outbound
            .send_request(&mut metered, session, cipher_kind, password.as_bytes())
            .await?;
        self.record_session_outbound_traffic(session.id, metered.drain_traffic());
        Ok(metered.into_inner().into())
    }

    #[cfg(not(feature = "outbound-shadowsocks"))]
    pub(crate) async fn connect_via_shadowsocks_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _password: &str,
        _cipher: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "shadowsocks-upstream".to_owned(),
            protocol: "shadowsocks",
            feature: "outbound-shadowsocks",
        })
    }

    #[cfg(feature = "outbound-trojan")]
    pub(crate) async fn connect_via_trojan_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(server, port, self.resolver.as_ref())
            .await?;
        let tls_config = ClientTlsConfig {
            server_name: sni.map(|s| s.to_owned()),
            disable_sni: false,
            ca_cert_path: None,
            insecure,
            alpn: vec![],
        };
        let tls_stream = zero_transport::tls::connect_tls_upstream(
            upstream,
            &tls_config,
            self.config.source_dir(),
            server,
        )
        .await?;
        let mut metered = crate::transport::MeteredStream::new(TcpRelayStream::new(tls_stream));
        self.protocols
            .trojan_outbound
            .send_request(&mut metered, session, password)
            .await?;
        self.record_session_outbound_traffic(session.id, metered.drain_traffic());
        Ok(metered.into_inner())
    }

    #[cfg(not(feature = "outbound-trojan"))]
    pub(crate) async fn connect_via_trojan_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _password: &str,
        _sni: Option<&str>,
        _insecure: bool,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "trojan-upstream".to_owned(),
            protocol: "trojan",
            feature: "outbound-trojan",
        })
    }
}
