//! TCP outbound establishment: routing pipeline and connection orchestration.
//!
//! Moved from transport/tcp_outbound.rs — these methods are runtime orchestration,
//! not transport I/O.

use std::io;
use std::sync::Arc;

use zero_core::Session;

use crate::runtime::upstream::VlessUpstream;
use crate::runtime::Proxy;
use crate::transport::{
    extract_tcp_stream, EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream, TcpRouteResult,
};
use zero_engine::{EngineError, EnginePlan};
use zero_engine::{ResolvedLeafOutbound, ResolvedOutbound};

impl Proxy {
    /// Execute the unified routing and outbound establishment pipeline.
    ///
    /// Caller MUST call `prepare_session` before this to assign a session ID.
    pub(crate) async fn route_and_establish_tcp(
        &self,
        session: &mut Session,
    ) -> Result<TcpRouteResult, EngineError> {
        self.resolve_fake_ip_target(session).await;
        let action = self.route_decision(session);
        let (resolved, _plan) = self.resolve_outbound(&action)?;
        let outbound = self
            .establish_tcp_outbound(session, (resolved, _plan))
            .await
            .map_err(|f| EngineError::Io(io::Error::other(f.error)))?;
        let mut result = extract_tcp_stream(outbound)?;
        result.route_action = action;
        Ok(result)
    }

    pub(crate) async fn establish_tcp_outbound(
        &self,
        session: &Session,
        resolved: (ResolvedOutbound<'static>, Option<Arc<EnginePlan>>),
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let (resolved, _plan) = resolved;
        match resolved {
            ResolvedOutbound::Relay { chain } => self.establish_relay_chain(session, chain).await,
            ResolvedOutbound::Single(candidate) => {
                self.establish_tcp_candidate(session, candidate).await
            }
            ResolvedOutbound::Fallback { candidates } => {
                let mut last_failure = None;

                for candidate in candidates {
                    match self.establish_tcp_candidate(session, candidate).await {
                        Ok(outbound) => return Ok(outbound),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure
                    .expect("validated fallback groups always have at least one candidate"))
            }
        }
    }

    pub(crate) async fn establish_tcp_candidate(
        &self,
        session: &Session,
        candidate: ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        // ── Kernel primitive: circuit breaker ──
        // Check health before connecting (skip for Direct / Block).
        let chained_tag = extract_chained_tag(&candidate);
        if let Some(ref tag) = chained_tag {
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
                match self
                    .connect_via_socks5_upstream(session, server, port, username.zip(password))
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
                split_http,
                quic,
            } => {
                match self
                    .connect_via_vless_upstream(
                        session,
                        VlessUpstream {
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
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                ..
            } => {
                match self
                    .connect_via_hysteria2_upstream(session, server, port, password)
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
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
            } => {
                match self
                    .connect_via_shadowsocks_upstream(session, server, port, password, cipher)
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
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
            } => {
                match self
                    .connect_via_trojan_upstream(session, server, port, password, sni, insecure)
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
            ResolvedLeafOutbound::Vmess {
                tag,
                server,
                port,
                id,
                cipher,
                tls,
                ws,
                grpc,
            } => {
                match self
                    .connect_via_vmess_upstream(session, server, port, id, cipher, tls, ws, grpc)
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
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                match self
                    .connect_via_mieru_upstream(session, server, port, username, password)
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
        };

        // ── Record health after connection attempt ──
        if let Some(ref tag) = chained_tag {
            match &result {
                Ok(_) => self.record_outbound_success(tag),
                Err(_) => self.record_outbound_failure(tag),
            }
        }

        result
    }

    /// Connect through a relay chain sequentially.
    async fn establish_relay_chain(
        &self,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let mut hops = chain.into_iter();
        let first = hops.next().expect("relay chain must have at least 2 hops");
        let second = hops.next().expect("relay chain must have at least 2 hops");

        let mut session_for_next = Session::new(
            0,
            hop_addr(&second),
            hop_port(&second),
            zero_core::Network::Tcp,
            zero_core::ProtocolType::Unknown,
        );

        let outbound = self
            .establish_tcp_candidate(&session_for_next, first)
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
                hop_addr(&next_hop),
                hop_port(&next_hop),
                zero_core::Network::Tcp,
                zero_core::ProtocolType::Unknown,
            );
            send_hop_protocol_request(self, &mut stream, &current_hop, &session_for_next)
                .await
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_hop",
                    error,
                    upstream_endpoint: None,
                })?;
            current_hop = next_hop;
        }

        send_hop_protocol_request(self, &mut stream, &current_hop, session)
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_last",
                error,
                upstream_endpoint: None,
            })?;

        Ok(EstablishedTcpOutbound::Relay { upstream: stream })
    }
}

/// Send a single hop's protocol request through an existing stream.
async fn send_hop_protocol_request(
    proxy: &Proxy,
    stream: &mut TcpRelayStream,
    hop: &ResolvedLeafOutbound<'_>,
    session: &Session,
) -> Result<(), EngineError> {
    match hop {
        #[cfg(feature = "socks5")]
        ResolvedLeafOutbound::Socks5 {
            username, password, ..
        } => proxy
            .protocols
            .socks5_outbound
            .establish_tunnel_with_auth(
                stream,
                session,
                username
                    .zip(*password)
                    .map(|(u, p)| zero_protocol_socks5::Socks5OutboundAuth {
                        username: u,
                        password: p,
                    }),
            )
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e))),
        #[cfg(feature = "vless")]
        ResolvedLeafOutbound::Vless { id, flow, .. } => {
            let uuid = zero_protocol_vless::parse_uuid(id)?;
            if let Some(f) = flow {
                proxy
                    .protocols
                    .vless_outbound
                    .establish_tcp_tunnel_with_flow(stream, session, &uuid, Some(f))
                    .await
                    .map_err(|e| EngineError::Io(std::io::Error::other(e)))
            } else {
                proxy
                    .protocols
                    .vless_outbound
                    .establish_tcp_tunnel(stream, session, &uuid)
                    .await
                    .map_err(|e| EngineError::Io(std::io::Error::other(e)))
            }
        }
        #[cfg(feature = "shadowsocks")]
        ResolvedLeafOutbound::Shadowsocks {
            password, cipher, ..
        } => {
            use zero_protocol_shadowsocks::{CipherKind, ShadowsocksOutbound};
            let kind = CipherKind::from_str(cipher).ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown ss cipher: {cipher}"),
                ))
            })?;
            ShadowsocksOutbound
                .send_request(stream, session, kind, password.as_bytes())
                .await
                .map(|_| ())
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))
        }
        #[cfg(feature = "trojan")]
        ResolvedLeafOutbound::Trojan { password, .. } => proxy
            .protocols
            .trojan_outbound
            .send_request(stream, session, password)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e))),
        #[cfg(feature = "vmess")]
        ResolvedLeafOutbound::Vmess { id, cipher, .. } => {
            let uuid = zero_protocol_vmess::parse_uuid(id).map_err(|e| {
                EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
            })?;
            let vmess_cipher =
                zero_protocol_vmess::VmessCipher::from_name(cipher).ok_or_else(|| {
                    EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("vmess unknown cipher: {cipher}"),
                    ))
                })?;
            zero_protocol_vmess::VmessOutbound
                .establish_tcp_tunnel(stream, session, &uuid, vmess_cipher)
                .await
                .map_err(|e| EngineError::Io(std::io::Error::other(e)))
        }
        #[cfg(feature = "mieru")]
        ResolvedLeafOutbound::Mieru { .. } => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "mieru relay hop is not supported yet",
        ))),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "relay hop protocol not supported or disabled",
        ))),
    }
}

/// Extract the outbound tag for health tracking.
/// Returns None for Direct and Block (always healthy).
fn extract_chained_tag(candidate: &ResolvedLeafOutbound<'_>) -> Option<String> {
    match candidate {
        ResolvedLeafOutbound::Direct { .. } | ResolvedLeafOutbound::Block { .. } => None,
        ResolvedLeafOutbound::Socks5 { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Vless { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Hysteria2 { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Shadowsocks { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Trojan { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Vmess { tag, .. } => Some(tag.to_string()),
        ResolvedLeafOutbound::Mieru { tag, .. } => Some(tag.to_string()),
    }
}

fn hop_addr(hop: &ResolvedLeafOutbound<'_>) -> zero_core::Address {
    use zero_core::Address;
    match hop {
        ResolvedLeafOutbound::Socks5 { server, .. }
        | ResolvedLeafOutbound::Vless { server, .. }
        | ResolvedLeafOutbound::Hysteria2 { server, .. }
        | ResolvedLeafOutbound::Shadowsocks { server, .. }
        | ResolvedLeafOutbound::Trojan { server, .. }
        | ResolvedLeafOutbound::Vmess { server, .. }
        | ResolvedLeafOutbound::Mieru { server, .. } => Address::Domain(server.to_string()),
        _ => Address::Domain("unknown".to_owned()),
    }
}

fn hop_port(hop: &ResolvedLeafOutbound<'_>) -> u16 {
    match hop {
        ResolvedLeafOutbound::Socks5 { port, .. }
        | ResolvedLeafOutbound::Vless { port, .. }
        | ResolvedLeafOutbound::Hysteria2 { port, .. }
        | ResolvedLeafOutbound::Shadowsocks { port, .. }
        | ResolvedLeafOutbound::Trojan { port, .. }
        | ResolvedLeafOutbound::Vmess { port, .. }
        | ResolvedLeafOutbound::Mieru { port, .. } => *port,
        _ => 0,
    }
}
