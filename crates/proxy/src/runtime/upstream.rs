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

        let connector = crate::transport::VlessTransportConnector::new(
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
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        use std::sync::Arc;
        use quinn::crypto::rustls::QuicClientConfig;
        use ring::hmac;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use zero_protocol_hysteria2::{
            build_auth_frame, build_tcp_connect_header, parse_auth_response,
        };
        use crate::transport::Hysteria2Stream;

        // ── QUIC connect ──
        // Use the same insecure verifier as VLESS QUIC transport
        #[derive(Debug)]
        struct SkipVerify;
        impl rustls::client::danger::ServerCertVerifier for SkipVerify {
            fn verify_server_cert(
                &self, _: &rustls::pki_types::CertificateDer<'_>,
                _: &[rustls::pki_types::CertificateDer<'_>],
                _: &rustls::pki_types::ServerName<'_>,
                _: &[u8], _: rustls::pki_types::UnixTime,
            ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
            fn verify_tls12_signature(
                &self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>,
                _: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn verify_tls13_signature(
                &self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>,
                _: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                vec![rustls::SignatureScheme::RSA_PKCS1_SHA256]
            }
        }

        let mut tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipVerify))
            .with_no_client_auth();

        tls_config.alpn_protocols = vec![b"hysteria2".to_vec()];

        let quic_cfg = QuicClientConfig::try_from(tls_config)
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic cfg: {e}"))))?;

        let mut client_cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
        let mut transport = quinn::TransportConfig::default();
        transport.max_idle_timeout(Some(
            std::time::Duration::from_secs(30).try_into().unwrap(),
        ));
        client_cfg.transport_config(Arc::new(transport));

        let bind_addr = "0.0.0.0:0"
            .parse::<std::net::SocketAddr>()
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic bind: {e}"))))?;

        let mut endpoint = quinn::Endpoint::client(bind_addr)
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic endpoint: {e}"))))?;
        endpoint.set_default_client_config(client_cfg);

        let server_addr = format!("{server}:{port}")
            .parse::<std::net::SocketAddr>()
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic addr: {e}"))))?;

        let conn = endpoint
            .connect(server_addr, server)
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic connect: {e}"))))?
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("quic connection: {e}"))))?;

        // ── HMAC auth ──
        // Salt: SHA256(server_addr + password), deterministic & matching server
        use ring::digest;
        let mut salt_ctx = digest::Context::new(&digest::SHA256);
        salt_ctx.update(server_addr.to_string().as_bytes());
        salt_ctx.update(b":");
        salt_ctx.update(password.as_bytes());
        let salt_digest = salt_ctx.finish();
        let salt: [u8; 32] = salt_digest.as_ref().try_into().unwrap();

        let key = hmac::Key::new(hmac::HMAC_SHA256, password.as_bytes());
        let auth_tag = hmac::sign(&key, &salt);
        let hmac_bytes: [u8; 32] = auth_tag.as_ref().try_into().unwrap();

        // ── Open bidirectional stream + auth ──
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {e}"))))?;

        let auth_frame = build_auth_frame(&hmac_bytes);
        send.write_all(&auth_frame)
            .await
            .map_err(|e| EngineError::Io(e.into()))?;
        send.flush().await.map_err(|e| EngineError::Io(e.into()))?;

        // Read auth response
        let mut resp_buf = [0u8; 32];
        let n = recv.read(&mut resp_buf)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 auth read: {e}"))))?
            .unwrap_or(0);
        parse_auth_response(&resp_buf[..n])
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 auth failed: {e}"))))?;

        // ── TCP connect ──
        let connect_header = build_tcp_connect_header(&session.target, session.port)
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 connect header: {e}"))))?;
        send.write_all(&connect_header)
            .await
            .map_err(|e| EngineError::Io(e.into()))?;
        send.flush().await.map_err(|e| EngineError::Io(e.into()))?;

        // Read connect response (1 byte: 0x01 = ok)
        let mut ok_buf = [0u8; 1];
        recv.read_exact(&mut ok_buf)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 connect read: {e}"))))?;
        if ok_buf[0] != 0x01 {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "hysteria2: connect rejected",
            )));
        }

        let stream = Hysteria2Stream::new(send, recv);
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
}
