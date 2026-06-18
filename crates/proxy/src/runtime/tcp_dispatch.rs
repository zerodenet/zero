//! TCP connection dispatch: routing pipeline and outbound orchestration.
//!
//! Moved from transport/tcp_outbound.rs; these methods are runtime orchestration,
//! not transport I/O.

use std::io;
use std::sync::Arc;

use zero_core::Session;
#[cfg(feature = "socks5")]
use zero_traits::TcpTunnelProtocol;

use crate::runtime::orchestration::{endpoint, health_tag, tcp_path_category, TcpPathCategory};
use crate::runtime::upstream::{
    Hysteria2Upstream, MieruUpstream, ShadowsocksUpstream, Socks5Upstream, TrojanUpstream,
    VlessUpstream,
};
use crate::runtime::Proxy;
use crate::transport::{
    extract_tcp_stream, EstablishedTcpOutbound, RelayCarrier, TcpOutboundFailure, TcpRelayStream,
    TcpRouteResult,
};
use zero_engine::{EngineError, EnginePlan};
use zero_engine::{ResolvedLeafOutbound, ResolvedOutbound};

impl Proxy {
    /// Execute the unified routing and outbound establishment pipeline.
    ///
    /// Caller MUST call `prepare_session` before this to assign a session ID.
    pub(crate) async fn dispatch_tcp(
        &self,
        session: &mut Session,
    ) -> Result<TcpRouteResult, EngineError> {
        self.resolve_fake_ip_target(session).await;
        let action = self.route_decision(session);
        let (resolved, _plan) = self.resolve_outbound(&action)?;
        let outbound = self
            .dispatch_tcp_outbound(session, (resolved, _plan))
            .await
            .map_err(|f| EngineError::Io(io::Error::other(f.error)))?;
        let mut result = extract_tcp_stream(outbound)?;
        result.route_action = action;
        Ok(result)
    }

    async fn dispatch_tcp_outbound(
        &self,
        session: &Session,
        resolved: (ResolvedOutbound<'static>, Option<Arc<EnginePlan>>),
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let (resolved, _plan) = resolved;
        match resolved {
            ResolvedOutbound::Relay { chain } => {
                self.dispatch_tcp_relay_chain(session, chain).await
            }
            ResolvedOutbound::Single(candidate) => {
                self.dispatch_tcp_candidate(session, candidate).await
            }
            ResolvedOutbound::Fallback { candidates } => {
                let mut last_failure = None;

                for candidate in candidates {
                    match self.dispatch_tcp_candidate(session, candidate).await {
                        Ok(outbound) => return Ok(outbound),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure
                    .expect("validated fallback groups always have at least one candidate"))
            }
        }
    }

    pub(crate) async fn dispatch_tcp_candidate(
        &self,
        session: &Session,
        candidate: ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        // Kernel primitive: circuit breaker.
        // Check health before connecting (skip for Direct / Block).
        let path_category = tcp_path_category(&candidate);
        let chained_tag = match path_category {
            TcpPathCategory::Direct | TcpPathCategory::Block => None,
            TcpPathCategory::Tunnel
            | TcpPathCategory::Session
            | TcpPathCategory::TransportSession => health_tag(&candidate).map(ToOwned::to_owned),
        };
        if let Some(tag) = chained_tag.as_deref() {
            if let Err(e) = self.check_outbound_health(tag) {
                return Err(TcpOutboundFailure {
                    stage: "health_check",
                    error: e,
                    upstream_endpoint: None,
                });
            }
        }

        let result = match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                match self
                    .protocols
                    .direct_outbound
                    .connect(session, self.resolver.as_ref())
                    .await
                {
                    Ok(upstream) => Ok(EstablishedTcpOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        upstream: upstream.into(),
                    }),
                    Err(error) => Err(TcpOutboundFailure {
                        stage: "connect_direct",
                        error: error.into(),
                        upstream_endpoint: None,
                    }),
                }
            }
            ResolvedLeafOutbound::Block { tag } => Ok(EstablishedTcpOutbound::Block {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                self.establish_socks5_outbound(session, tag, server, port, username, password)
                    .await
            }
            ResolvedLeafOutbound::Vless {
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
                quic,
                split_http,
            } => {
                self.establish_vless_outbound(
                    session,
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
                    quic,
                    split_http,
                )
                .await
            }
            #[cfg(feature = "hysteria2")]
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                insecure,
                client_fingerprint,
            } => {
                self.establish_hysteria2_outbound(
                    session,
                    tag,
                    server,
                    port,
                    password,
                    insecure,
                    client_fingerprint,
                )
                .await
            }
            #[cfg(feature = "shadowsocks")]
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
            } => {
                self.establish_shadowsocks_outbound(session, tag, server, port, password, cipher)
                    .await
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                self.establish_trojan_outbound(
                    session,
                    tag,
                    server,
                    port,
                    password,
                    sni,
                    insecure,
                    client_fingerprint,
                )
                .await
            }
            #[cfg(feature = "vmess")]
            ResolvedLeafOutbound::Vmess {
                tag,
                server,
                port,
                id,
                cipher,
                mux_concurrency,
                mux_idle_timeout_secs,
                tls,
                ws,
                grpc,
            } => {
                self.establish_vmess_outbound(
                    session,
                    tag,
                    server,
                    port,
                    id,
                    cipher,
                    mux_concurrency,
                    mux_idle_timeout_secs,
                    tls,
                    ws,
                    grpc,
                )
                .await
            }
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                self.establish_mieru_outbound(session, tag, server, port, username, password)
                    .await
            }
            #[allow(unreachable_patterns)]
            _ => Err(TcpOutboundFailure {
                stage: "protocol_not_compiled",
                error: EngineError::Io(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "protocol not compiled in this build",
                )),
                upstream_endpoint: None,
            }),
        };

        // Record health after connection attempt.
        if let Some(tag) = chained_tag.as_deref() {
            match &result {
                Ok(_) => self.record_outbound_success(tag),
                Err(_) => self.record_outbound_failure(tag),
            }
        }

        result
    }

    /// Dispatch through a relay chain sequentially.
    async fn establish_socks5_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_socks5_upstream(
                session,
                Socks5Upstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
                    auth: username.zip(password),
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Socks5 {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_socks5",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "vless")]
    async fn establish_vless_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        flow: Option<&str>,
        mux_concurrency: Option<u32>,
        mux_idle_timeout_secs: Option<u64>,
        tls: Option<&zero_config::ClientTlsConfig>,
        reality: Option<&zero_config::RealityConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
        h2: Option<&zero_config::H2Config>,
        http_upgrade: Option<&zero_config::HttpUpgradeConfig>,
        quic: Option<&zero_config::QuicConfig>,
        split_http: Option<&zero_config::SplitHttpConfig>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_vless_upstream(
                session,
                VlessUpstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
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
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vless {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vless",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "hysteria2")]
    async fn establish_hysteria2_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        password: &str,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_hysteria2_upstream(
                session,
                crate::runtime::upstream::Hysteria2Upstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
                    password,
                    client_fingerprint,
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Hysteria2 {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_hysteria2",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "shadowsocks")]
    async fn establish_shadowsocks_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        password: &str,
        cipher: &str,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_shadowsocks_upstream(
                session,
                crate::runtime::upstream::ShadowsocksUpstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
                    password,
                    cipher,
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Shadowsocks {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_shadowsocks",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "trojan")]
    async fn establish_trojan_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_trojan_upstream(
                session,
                crate::runtime::upstream::TrojanUpstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
                    password,
                    sni,
                    insecure,
                    client_fingerprint,
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Trojan {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_trojan",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "vmess")]
    async fn establish_vmess_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        mux_idle_timeout_secs: Option<u64>,
        tls: Option<&zero_config::ClientTlsConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_vmess_upstream(
                session,
                server,
                port,
                id,
                cipher,
                mux_concurrency,
                mux_idle_timeout_secs,
                tls,
                ws,
                grpc,
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vmess {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vmess",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    #[cfg(feature = "mieru")]
    async fn establish_mieru_outbound(
        &self,
        session: &Session,
        tag: &str,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self
            .connect_via_mieru_upstream(
                session,
                crate::runtime::upstream::MieruUpstream {
                    endpoint: crate::runtime::orchestration::OutboundEndpoint { server, port },
                    username,
                    password,
                },
            )
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Mieru {
                tag: tag.to_owned(),
                server: server.to_owned(),
                port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_mieru",
                error,
                upstream_endpoint: Some((server.to_owned(), port)),
            }),
        }
    }

    async fn dispatch_tcp_relay_chain<'a>(
        &self,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'a>>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let (carrier, final_hop) = self.dispatch_tcp_relay_prefix(chain).await?;

        let stream = apply_hop_protocol(self, carrier.stream, &final_hop, session)
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_last",
                error,
                upstream_endpoint: None,
            })?;

        Ok(EstablishedTcpOutbound::Relay { upstream: stream })
    }

    /// Establish all relay hops before the final protocol hop.
    ///
    /// The returned stream is connected to the final hop server through the
    /// preceding relay hops. The caller is responsible for running the final
    /// hop protocol handshake on that stream.
    pub(crate) async fn dispatch_tcp_relay_prefix<'a>(
        &self,
        chain: Vec<ResolvedLeafOutbound<'a>>,
    ) -> Result<(RelayCarrier, ResolvedLeafOutbound<'a>), TcpOutboundFailure> {
        let mut hops = chain.into_iter();
        let first = hops.next().expect("relay chain must have at least 2 hops");
        let second = hops.next().expect("relay chain must have at least 2 hops");

        let mut session_for_next = Session::new(
            0,
            endpoint(&second)
                .map(|endpoint| endpoint.address())
                .unwrap_or_else(|| zero_core::Address::Domain("unknown".to_owned())),
            endpoint(&second).map(|endpoint| endpoint.port).unwrap_or(0),
            zero_core::Network::Tcp,
            zero_core::ProtocolType::Unknown,
        );

        let outbound = self
            .dispatch_tcp_candidate(&session_for_next, first)
            .await?;
        let mut stream = match outbound {
            EstablishedTcpOutbound::Direct { upstream, .. }
            | EstablishedTcpOutbound::Socks5 { upstream, .. }
            | EstablishedTcpOutbound::Vless { upstream, .. }
            | EstablishedTcpOutbound::Hysteria2 { upstream, .. }
            | EstablishedTcpOutbound::Shadowsocks { upstream, .. }
            | EstablishedTcpOutbound::Trojan { upstream, .. }
            | EstablishedTcpOutbound::Vmess { upstream, .. }
            | EstablishedTcpOutbound::Mieru { upstream, .. }
            | EstablishedTcpOutbound::Relay { upstream } => upstream,
            EstablishedTcpOutbound::Block { .. } => {
                return Err(TcpOutboundFailure {
                    stage: "relay_first_hop",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "first relay hop resolved to block",
                    )),
                    upstream_endpoint: None,
                })
            }
        };

        let mut current_hop = second;
        for next_hop in hops {
            session_for_next = Session::new(
                0,
                endpoint(&next_hop)
                    .map(|endpoint| endpoint.address())
                    .unwrap_or_else(|| zero_core::Address::Domain("unknown".to_owned())),
                endpoint(&next_hop)
                    .map(|endpoint| endpoint.port)
                    .unwrap_or(0),
                zero_core::Network::Tcp,
                zero_core::ProtocolType::Unknown,
            );
            stream = apply_hop_protocol(self, stream, &current_hop, &session_for_next)
                .await
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_hop",
                    error,
                    upstream_endpoint: None,
                })?;
            current_hop = next_hop;
        }

        let ep = endpoint(&current_hop).expect("final relay hop must have an endpoint");
        Ok((
            RelayCarrier {
                stream,
                server: ep.server.to_owned(),
                port: ep.port,
            },
            current_hop,
        ))
    }
}

/// Apply a single hop's protocol request to an existing stream.
///
/// Most relay protocols only write a tunnel request and keep using the same
/// stream. Protocols with post-handshake session state return a wrapped stream
/// that owns that state.
async fn apply_hop_protocol(
    proxy: &Proxy,
    mut stream: TcpRelayStream,
    hop: &ResolvedLeafOutbound<'_>,
    session: &Session,
) -> Result<TcpRelayStream, EngineError> {
    match hop {
        #[cfg(feature = "socks5")]
        ResolvedLeafOutbound::Socks5 {
            username, password, ..
        } => {
            proxy
                .protocols
                .socks5_outbound
                .establish_tcp_tunnel(
                    &mut stream,
                    &socks5::Socks5TcpTunnelTarget {
                        session,
                        auth: username
                            .zip(*password)
                            .map(|(u, p)| socks5::Socks5OutboundAuth {
                                username: u,
                                password: p,
                            }),
                    },
                )
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            Ok(stream)
        }
        #[cfg(feature = "vless")]
        ResolvedLeafOutbound::Vless { id, flow, .. } => {
            let uuid = vless::parse_uuid(id)?;
            if flow.is_some() {
                use zero_traits::TcpTunnelProtocol;
                <vless::VlessOutbound as TcpTunnelProtocol<
                    vless::VlessFlowTcpTunnelTarget,
                >>::establish_tcp_tunnel(
                    &proxy.protocols.vless_outbound,
                    &mut stream,
                    &vless::VlessFlowTcpTunnelTarget {
                        session,
                        id: &uuid,
                        flow: flow.map(|f| f),
                    },
                )
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            } else {
                use zero_traits::TcpTunnelProtocol;
                <vless::VlessOutbound as TcpTunnelProtocol<
                    vless::VlessTcpTunnelTarget,
                >>::establish_tcp_tunnel(
                    &proxy.protocols.vless_outbound,
                    &mut stream,
                    &vless::VlessTcpTunnelTarget { session, id: &uuid },
                )
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            };
            Ok(stream)
        }
        #[cfg(feature = "shadowsocks")]
        ResolvedLeafOutbound::Shadowsocks {
            password, cipher, ..
        } => {
            use shadowsocks::{CipherKind, ShadowsocksOutbound};
            use zero_traits::TcpSessionProtocol;
            let kind = CipherKind::from_str(cipher).ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown ss cipher: {cipher}"),
                ))
            })?;
            let ss_session = <ShadowsocksOutbound as TcpSessionProtocol<
                shadowsocks::ShadowsocksTcpTarget,
            >>::establish_tcp_session(
                &ShadowsocksOutbound,
                &mut stream,
                &shadowsocks::ShadowsocksTcpTarget {
                    session,
                    cipher: kind,
                    password: password.as_bytes(),
                },
            )
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            Ok(crate::runtime::upstream::wrap_shadowsocks_outbound_stream(
                stream,
                ss_session,
                password.as_bytes().to_vec(),
            ))
        }
        #[cfg(feature = "trojan")]
        ResolvedLeafOutbound::Trojan { password, .. } => {
            proxy
                .protocols
                .trojan_outbound
                .establish_tcp_tunnel(
                    &mut stream,
                    &trojan::TrojanTcpTunnelTarget { session, password },
                )
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            Ok(stream)
        }
        #[cfg(feature = "vmess")]
        ResolvedLeafOutbound::Vmess { id, cipher, .. } => {
            let uuid = vmess::parse_uuid(id).map_err(|e| {
                EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
            })?;
            let vmess_cipher = vmess::VmessCipher::from_name(cipher).ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("vmess unknown cipher: {cipher}"),
                ))
            })?;
            use zero_traits::TcpSessionProtocol;
            let vmess_session = <vmess::VmessOutbound as TcpSessionProtocol<
                vmess::VmessTcpSessionTarget,
            >>::establish_tcp_session(
                &vmess::VmessOutbound,
                &mut stream,
                &vmess::VmessTcpSessionTarget {
                    session,
                    uuid: &uuid,
                    cipher: vmess_cipher,
                },
            )
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            Ok(TcpRelayStream::new(vmess::VmessAeadStream::outbound(
                stream,
                vmess_session,
            )?))
        }
        #[cfg(feature = "mieru")]
        ResolvedLeafOutbound::Mieru {
            username, password, ..
        } => {
            use zero_traits::TcpSessionProtocol;
            let outbound =
                <mieru::MieruProtocol as TcpSessionProtocol<mieru::MieruTcpTarget>>::establish_tcp_session(
                    &mieru::MieruProtocol,
                    &mut stream,
                    &mieru::MieruTcpTarget { username, password },
                )
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            let mut mieru_stream = crate::outbound::mieru::MieruTcpStream::new(stream, outbound);
            crate::outbound::mieru::socks5_connect(
                &mut mieru_stream,
                &session.target,
                session.port,
            )
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
            Ok(TcpRelayStream::new(mieru_stream))
        }
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "relay hop protocol not supported or disabled",
        ))),
    }
}
