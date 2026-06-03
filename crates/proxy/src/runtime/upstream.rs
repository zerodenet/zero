use zero_config::{ClientTlsConfig, GrpcConfig, H2Config, QuicConfig, RealityConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

#[cfg(feature = "trojan")]
use tokio::io::AsyncWriteExt;

use crate::runtime::Proxy;
#[cfg(feature = "socks5")]
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
    #[cfg(feature = "socks5")]
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

    #[cfg(not(feature = "socks5"))]
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
            feature: "socks5",
        })
    }

    #[cfg(feature = "vless")]
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

    #[cfg(not(feature = "vless"))]
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
            feature: "vless",
        })
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn connect_via_hysteria2_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
    ) -> Result<TcpRelayStream, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(server, port, password)
            .with_fingerprint(client_fingerprint);
        let stream = connector.connect(session).await?;
        Ok(TcpRelayStream::new(stream))
    }

    #[cfg(not(feature = "hysteria2"))]
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
            feature: "hysteria2",
        })
    }

    #[cfg(feature = "shadowsocks")]
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
        let password_bytes = password.as_bytes().to_vec();
        let ss_session = self
            .protocols
            .shadowsocks_outbound
            .send_request(&mut metered, session, cipher_kind, &password_bytes)
            .await?;
        self.record_session_outbound_traffic(session.id, metered.drain_traffic());
        let upstream = metered.into_inner();
        let (app_stream, ss_plain_stream) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let _ = relay_shadowsocks_outbound(
                ss_plain_stream,
                upstream.into(),
                ss_session,
                password_bytes,
            )
            .await;
        });
        Ok(crate::transport::TcpRelayStream::new(app_stream))
    }

    #[cfg(not(feature = "shadowsocks"))]
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
            feature: "shadowsocks",
        })
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn connect_via_vmess_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        tls: Option<&zero_config::ClientTlsConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
    ) -> Result<TcpRelayStream, EngineError> {
        use zero_protocol_vmess::{parse_uuid, VmessCipher, VmessOutbound};

        let uuid = parse_uuid(id).map_err(|e| {
            EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
        })?;
        let vmess_cipher = VmessCipher::from_name(cipher).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("vmess unknown cipher: {cipher}"),
            ))
        })?;

        let socket = self
            .protocols
            .direct_outbound
            .connect_host(server, port, self.resolver.as_ref())
            .await?;

        // Transport stack: gRPC > WS > TLS > raw
        let stream: TcpRelayStream = match (grpc, ws, tls) {
            (Some(grpc_cfg), None, Some(tls_cfg)) => {
                let tls_stream = zero_transport::tls::connect_tls_upstream(
                    socket,
                    tls_cfg,
                    self.config.source_dir(),
                    server,
                )
                .await?;
                TcpRelayStream::new(
                    zero_transport::grpc::connect_grpc(tls_stream, &grpc_cfg.service_names).await?,
                )
            }
            (Some(grpc_cfg), None, None) => TcpRelayStream::new(
                zero_transport::grpc::connect_grpc(socket, &grpc_cfg.service_names).await?,
            ),
            (None, Some(ws_cfg), Some(tls_cfg)) => {
                let tls_stream = zero_transport::tls::connect_tls_upstream(
                    socket,
                    tls_cfg,
                    self.config.source_dir(),
                    server,
                )
                .await?;
                TcpRelayStream::new(
                    zero_transport::ws::connect_ws(tls_stream, ws_cfg, server, port).await?,
                )
            }
            (None, Some(ws_cfg), None) => TcpRelayStream::new(
                zero_transport::ws::connect_ws(socket, ws_cfg, server, port).await?,
            ),
            (None, None, Some(tls_cfg)) => {
                let tls_stream = zero_transport::tls::connect_tls_upstream(
                    socket,
                    tls_cfg,
                    self.config.source_dir(),
                    server,
                )
                .await?;
                TcpRelayStream::new(tls_stream)
            }
            (None, None, None) => TcpRelayStream::new(socket),
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "vmess: ws, grpc, and tls are mutually exclusive for transport",
                )))
            }
        };

        let mut sock = crate::transport::MeteredStream::new(stream);
        VmessOutbound
            .establish_tcp_tunnel(&mut sock, session, &uuid, vmess_cipher)
            .await?;
        self.record_session_outbound_traffic(session.id, sock.drain_traffic());
        Ok(sock.into_inner())
    }

    #[cfg(not(feature = "vmess"))]
    pub(crate) async fn connect_via_vmess_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _id: &str,
        _cipher: &str,
        _tls: Option<&zero_config::ClientTlsConfig>,
        _ws: Option<&zero_config::WebSocketConfig>,
        _grpc: Option<&zero_config::GrpcConfig>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "vmess-upstream".to_owned(),
            protocol: "vmess",
            feature: "vmess",
        })
    }

    #[cfg(feature = "trojan")]
    pub(crate) async fn connect_via_trojan_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
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
            alpn: Vec::new(),
            client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
        };
        let tls_stream = zero_transport::tls::connect_tls_upstream(
            upstream,
            &tls_config,
            self.config.source_dir(),
            server,
        )
        .await?;
        let mut metered = crate::transport::MeteredStream::new(tls_stream);
        self.protocols
            .trojan_outbound
            .send_request(&mut metered, session, password)
            .await?;
        metered.flush().await?;
        let traffic = metered.drain_traffic();
        tracing::debug!(
            session_id = session.id,
            trojan_handshake_tx = traffic.written_bytes,
            target = ?session.target,
            target_port = session.port,
            "trojan upstream connected"
        );
        self.record_session_outbound_traffic(session.id, traffic);
        Ok(metered.into_inner())
    }

    #[cfg(not(feature = "trojan"))]
    pub(crate) async fn connect_via_trojan_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _password: &str,
        _sni: Option<&str>,
        _insecure: bool,
        _client_fingerprint: Option<&str>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "trojan-upstream".to_owned(),
            protocol: "trojan",
            feature: "trojan",
        })
    }

    /// Mieru upstream (stub — feature disabled).
    #[cfg(not(feature = "mieru"))]
    pub(crate) async fn connect_via_mieru_upstream(
        &self,
        _session: &Session,
        _server: &str,
        _port: u16,
        _username: &str,
        _password: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "mieru-upstream".to_owned(),
            protocol: "mieru",
            feature: "mieru",
        })
    }

    /// Mieru upstream — connect + handshake, return raw TCP stream.
    #[cfg(feature = "mieru")]
    pub(crate) async fn connect_via_mieru_upstream(
        &self,
        session: &Session,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Result<TcpRelayStream, EngineError> {
        let socket = self
            .protocols
            .direct_outbound
            .connect_host(server, port, self.resolver.as_ref())
            .await?;

        // Wrap in TcpRelayStream for AsyncSocket compatibility
        let mut stream = TcpRelayStream::new(socket);

        let outbound = zero_protocol_mieru::MieruOutbound::connect(
            &mut stream,
            username,
            password,
            &session.target,
            session.port,
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru handshake: {e}"))))?;

        Ok(TcpRelayStream::new(
            crate::outbound::mieru::MieruTcpStream::new(stream, outbound),
        ))
    }
}

#[cfg(feature = "shadowsocks")]
async fn relay_shadowsocks_outbound(
    app_stream: tokio::io::DuplexStream,
    upstream: TcpRelayStream,
    ss_session: zero_protocol_shadowsocks::ShadowsocksOutboundSession,
    password: Vec<u8>,
) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use zero_protocol_shadowsocks::{
        decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload, encrypt_tcp_chunk,
    };

    let cipher = ss_session.cipher;
    let (mut app_read, mut app_write) = tokio::io::split(app_stream);
    let (mut upstream_read, mut upstream_write) = tokio::io::split(upstream);

    let upload_key = ss_session.session_key;
    let mut upload_nonce = ss_session.next_upload_nonce;
    let upload = tokio::spawn(async move {
        let mut buf = [0u8; 0x3fff];
        loop {
            let n = app_read.read(&mut buf).await?;
            if n == 0 {
                let _ = upstream_write.shutdown().await;
                return Ok::<(), std::io::Error>(());
            }
            let chunk = encrypt_tcp_chunk(cipher, &upload_key, &mut upload_nonce, &buf[..n])
                .map_err(protocol_to_io)?;
            upstream_write.write_all(&chunk).await?;
            upstream_write.flush().await?;
        }
    });

    let download = tokio::spawn(async move {
        let mut salt = vec![0u8; cipher.salt_len()];
        upstream_read.read_exact(&mut salt).await?;
        let download_key = ss_derive_outbound_key(cipher, &password, &salt)?;
        let mut download_nonce = 0;

        loop {
            let mut encrypted_length = vec![0u8; 2 + cipher.tag_len()];
            if upstream_read
                .read_exact(&mut encrypted_length)
                .await
                .is_err()
            {
                let _ = app_write.shutdown().await;
                return Ok::<(), std::io::Error>(());
            }
            let payload_len = decrypt_tcp_chunk_length(
                cipher,
                &download_key,
                &mut download_nonce,
                &encrypted_length,
            )
            .map_err(protocol_to_io)?;
            let mut encrypted_payload = vec![0u8; payload_len + cipher.tag_len()];
            upstream_read.read_exact(&mut encrypted_payload).await?;
            let plain = decrypt_tcp_chunk_payload(
                cipher,
                &download_key,
                &mut download_nonce,
                payload_len,
                &encrypted_payload,
            )
            .map_err(protocol_to_io)?;
            app_write.write_all(&plain).await?;
            app_write.flush().await?;
        }
    });

    let _ = tokio::try_join!(upload, download);
    Ok(())
}

#[cfg(feature = "shadowsocks")]
fn ss_derive_outbound_key(
    cipher: zero_protocol_shadowsocks::CipherKind,
    password: &[u8],
    salt: &[u8],
) -> std::io::Result<Vec<u8>> {
    if cipher.is_blake3() {
        zero_protocol_shadowsocks::derive_key_blake3(password, salt, cipher.key_len())
            .map_err(protocol_to_io)
    } else {
        zero_protocol_shadowsocks::derive_key(password, salt, cipher.key_len())
            .map_err(protocol_to_io)
    }
}

#[cfg(feature = "shadowsocks")]
fn protocol_to_io(error: zero_core::Error) -> std::io::Error {
    std::io::Error::other(error.to_string())
}
