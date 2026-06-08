use zero_config::{ClientTlsConfig, GrpcConfig, H2Config, QuicConfig, RealityConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;
#[cfg(feature = "socks5")]
use zero_traits::TcpTunnelProtocol;

#[cfg(feature = "trojan")]
use tokio::io::AsyncWriteExt;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
#[cfg(feature = "socks5")]
use crate::transport::MeteredStream;
use crate::transport::TcpRelayStream;

pub(crate) struct Socks5Upstream<'a> {
    pub(crate) endpoint: OutboundEndpoint<'a>,
    pub(crate) auth: Option<(&'a str, &'a str)>,
}

pub(crate) struct VlessUpstream<'a> {
    pub endpoint: OutboundEndpoint<'a>,
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

pub(crate) struct Hysteria2Upstream<'a> {
    pub(crate) endpoint: OutboundEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) client_fingerprint: Option<&'a str>,
}

pub(crate) struct ShadowsocksUpstream<'a> {
    pub(crate) endpoint: OutboundEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) cipher: &'a str,
}

pub(crate) struct TrojanUpstream<'a> {
    pub(crate) endpoint: OutboundEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
}

pub(crate) struct MieruUpstream<'a> {
    pub(crate) endpoint: OutboundEndpoint<'a>,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
}

impl Proxy {
    #[cfg(feature = "socks5")]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        session: &Session,
        peer: Socks5Upstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                self.resolver.as_ref(),
            )
            .await?;
        let mut upstream = MeteredStream::new(upstream);

        self.protocols
            .socks5_outbound
            .establish_tcp_tunnel(
                &mut upstream,
                &socks5::Socks5TcpTunnelTarget {
                    session,
                    auth: peer
                        .auth
                        .map(|(username, password)| socks5::Socks5OutboundAuth {
                            username,
                            password,
                        }),
                },
            )
            .await?;
        self.record_session_outbound_traffic(session.id, upstream.drain_traffic());

        Ok(upstream.into_inner().into())
    }

    #[cfg(not(feature = "socks5"))]
    pub(crate) async fn connect_via_socks5_upstream(
        &self,
        _session: &Session,
        _upstream: Socks5Upstream<'_>,
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
        let id = vless::parse_uuid(upstream.id)?;

        // If MUX flow is configured, use connection pool
        if upstream.flow == Some("xtls-rprx-vision") {
            return self
                .mux_pool
                .open_stream(
                    self,
                    session,
                    upstream.endpoint.server.to_owned(),
                    upstream.endpoint.port,
                    &id,
                    upstream.tls,
                    upstream.reality,
                    upstream.mux_concurrency.unwrap_or(8),
                    upstream.mux_idle_timeout_secs.unwrap_or(300),
                )
                .await;
        }

        // QUIC uses UDP; handle before TCP connect entirely.
        if let Some(quic) = upstream.quic {
            let server_name = quic
                .server_name
                .as_deref()
                .unwrap_or(upstream.endpoint.server);
            let quic_stream =
                crate::transport::connect_quic(server_name, upstream.endpoint.port, quic.insecure)
                    .await?;
            return Ok(TcpRelayStream::new(quic_stream));
        }

        let socket = self
            .protocols
            .direct_outbound
            .connect_host(
                upstream.endpoint.server,
                upstream.endpoint.port,
                self.resolver.as_ref(),
            )
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
            .connect(socket, upstream.endpoint.server, upstream.endpoint.port)
            .await?;

        let flow = upstream.flow;
        let is_reality = upstream.reality.is_some();
        let mut metered = crate::transport::MeteredStream::new(stream);

        if is_reality {
            use zero_traits::DeferredTcpTunnelProtocol;
            self.protocols
                .vless_outbound
                .send_deferred_tcp_tunnel_request(
                    &mut metered,
                    &vless::VlessFlowTcpTunnelTarget {
                        session,
                        id: &id,
                        flow,
                    },
                )
                .await?;
            self.record_session_outbound_traffic(session.id, metered.drain_traffic());

            Ok(TcpRelayStream::new(
                vless::DeferredVlessResponseStream::new(metered.into_inner()),
            ))
        } else {
            use zero_traits::TcpTunnelProtocol;
            <vless::VlessOutbound as TcpTunnelProtocol<
                vless::VlessFlowTcpTunnelTarget,
            >>::establish_tcp_tunnel(
                &self.protocols.vless_outbound,
                &mut metered,
                &vless::VlessFlowTcpTunnelTarget { session, id: &id, flow },
            )
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
            upstream.endpoint.server,
            upstream.endpoint.port,
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
        peer: Hysteria2Upstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(
            peer.endpoint.server,
            peer.endpoint.port,
            peer.password,
        )
        .with_fingerprint(peer.client_fingerprint);
        let stream = connector.connect(session).await?;
        Ok(TcpRelayStream::new(stream))
    }

    #[cfg(not(feature = "hysteria2"))]
    pub(crate) async fn connect_via_hysteria2_upstream(
        &self,
        _session: &Session,
        _peer: Hysteria2Upstream<'_>,
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
        peer: ShadowsocksUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        use shadowsocks::CipherKind;

        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                self.resolver.as_ref(),
            )
            .await?;
        let mut metered = crate::transport::MeteredStream::new(upstream);
        let cipher_kind = CipherKind::from_str(peer.cipher).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown shadowsocks cipher: {}", peer.cipher),
            ))
        })?;
        let password_bytes = peer.password.as_bytes().to_vec();
        use zero_traits::TcpSessionProtocol;
        let ss_session = <shadowsocks::ShadowsocksOutbound as TcpSessionProtocol<
            shadowsocks::ShadowsocksTcpTarget,
        >>::establish_tcp_session(
            &self.protocols.shadowsocks_outbound,
            &mut metered,
            &shadowsocks::ShadowsocksTcpTarget {
                session,
                cipher: cipher_kind,
                password: &password_bytes,
            },
        )
        .await?;
        self.record_session_outbound_traffic(session.id, metered.drain_traffic());
        Ok(wrap_shadowsocks_outbound_stream(
            metered.into_inner().into(),
            ss_session,
            password_bytes,
        ))
    }

    #[cfg(not(feature = "shadowsocks"))]
    pub(crate) async fn connect_via_shadowsocks_upstream(
        &self,
        _session: &Session,
        _peer: ShadowsocksUpstream<'_>,
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
        use vmess::{parse_uuid, VmessCipher, VmessOutbound};
        use zero_traits::TcpTunnelProtocol;

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
        <VmessOutbound as TcpTunnelProtocol<vmess::VmessTcpTunnelTarget>>::establish_tcp_tunnel(
            &VmessOutbound,
            &mut sock,
            &vmess::VmessTcpTunnelTarget {
                session,
                uuid: &uuid,
                cipher: vmess_cipher,
            },
        )
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
        peer: TrojanUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let upstream = self
            .protocols
            .direct_outbound
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                self.resolver.as_ref(),
            )
            .await?;
        let tls_config = ClientTlsConfig {
            server_name: peer.sni.map(|s| s.to_owned()),
            disable_sni: false,
            ca_cert_path: None,
            insecure: peer.insecure,
            alpn: Vec::new(),
            client_fingerprint: peer.client_fingerprint.map(|s| s.to_owned()),
        };
        let tls_stream = zero_transport::tls::connect_tls_upstream(
            upstream,
            &tls_config,
            self.config.source_dir(),
            peer.endpoint.server,
        )
        .await?;
        let mut metered = crate::transport::MeteredStream::new(tls_stream);
        self.protocols
            .trojan_outbound
            .establish_tcp_tunnel(
                &mut metered,
                &trojan::TrojanTcpTunnelTarget {
                    session,
                    password: peer.password,
                },
            )
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
        _peer: TrojanUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "trojan-upstream".to_owned(),
            protocol: "trojan",
            feature: "trojan",
        })
    }

    /// Mieru upstream stub for disabled feature builds.
    #[cfg(not(feature = "mieru"))]
    pub(crate) async fn connect_via_mieru_upstream(
        &self,
        _session: &Session,
        _peer: MieruUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        Err(EngineError::CompiledFeatureDisabled {
            kind: "outbound",
            tag: "mieru-upstream".to_owned(),
            protocol: "mieru",
            feature: "mieru",
        })
    }

    /// Mieru upstream: connect and handshake, return raw TCP stream.
    #[cfg(feature = "mieru")]
    pub(crate) async fn connect_via_mieru_upstream(
        &self,
        session: &Session,
        peer: MieruUpstream<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let socket = self
            .protocols
            .direct_outbound
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                self.resolver.as_ref(),
            )
            .await?;

        // Wrap in TcpRelayStream for AsyncSocket compatibility
        let mut stream = TcpRelayStream::new(socket);

        use zero_traits::TcpSessionProtocol;
        let outbound = <mieru::MieruProtocol as TcpSessionProtocol<mieru::MieruTcpTarget>>::establish_tcp_session(
            &mieru::MieruProtocol,
                &mut stream,
                &mieru::MieruTcpTarget {
                    target: &session.target,
                    port: session.port,
                    username: peer.username,
                    password: peer.password,
                },
            )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru handshake: {e}"))))?;

        Ok(TcpRelayStream::new(
            crate::outbound::mieru::MieruTcpStream::new(stream, outbound),
        ))
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn wrap_shadowsocks_outbound_stream(
    upstream: TcpRelayStream,
    ss_session: shadowsocks::ShadowsocksOutboundSession,
    password: Vec<u8>,
) -> TcpRelayStream {
    TcpRelayStream::new(shadowsocks::ShadowsocksAeadStream::outbound(
        upstream, ss_session, password,
    ))
}
