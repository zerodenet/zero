//! Hysteria2 inbound: QUIC accept, HMAC auth, TCP stream dispatch.
//!
//! TCP stream relay uses the `InboundProtocol` trait with a custom relay
//! that handles QUIC stream I/O (not raw TCP).

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use hysteria2::{Hysteria2Inbound, Hysteria2InboundProfile};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{Address, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::Proxy;
use crate::transport::{copy_one_way, Hysteria2Stream};

#[derive(Debug)]
pub(crate) struct Hysteria2InboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: Hysteria2InboundProfile,
    pub(crate) up_bps: Option<u64>,
    pub(crate) down_bps: Option<u64>,
}

// ── Handler for individual TCP streams ─────────────────────────────────

/// Handler for a single Hysteria2 TCP stream (QUIC bi-directional stream).
///
/// The QUIC connection lifecycle (auth, datagram loop) is managed by the
/// listener.  This handler only deals with individual TCP streams.
#[derive(Clone)]
pub(crate) struct Hysteria2StreamHandler;

#[async_trait]
impl InboundProtocol for Hysteria2StreamHandler {
    type ClientStream = Hysteria2Stream;

    async fn accept(
        &self,
        _stream: crate::transport::TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // Hysteria2 accept is handled inline by the listener; this is unused.
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "Hysteria2 accept is handled by the listener",
        )))
    }

    async fn send_ok(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        let ok = Hysteria2Inbound.connect_ok_response();
        AsyncSocket::write_all(client, &ok)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("write connect ok: {e}"))))
    }

    async fn send_blocked(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        let err = Hysteria2Inbound.connect_error_response("blocked");
        let _ = AsyncSocket::write_all(client, &err).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        let err = Hysteria2Inbound.connect_error_response("outbound failed");
        let _ = AsyncSocket::write_all(client, &err).await;
        Ok(())
    }

    /// QUIC stream relay: `copy_one_way` × 2 (not raw TCP relay).
    async fn relay(
        &self,
        client: Hysteria2Stream,
        upstream: crate::transport::TcpRelayStream,
        proxy: &Proxy,
        session_id: u64,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        let (up_read, up_write) = tokio::io::split(upstream);
        let (down_read, down_write) = tokio::io::split(client);

        let upload_proxy = proxy.clone();
        let upload = tokio::spawn(async move {
            let _ = copy_one_way(
                down_read,
                up_write,
                |bytes| {
                    upload_proxy.record_session_inbound_rx(session_id, bytes);
                    upload_proxy.record_session_outbound_tx(session_id, bytes);
                },
                up_bps,
            )
            .await;
        });
        let download_proxy = proxy.clone();
        let download = tokio::spawn(async move {
            let _ = copy_one_way(
                up_read,
                down_write,
                |bytes| {
                    download_proxy.record_session_outbound_rx(session_id, bytes);
                    download_proxy.record_session_inbound_tx(session_id, bytes);
                },
                down_bps,
            )
            .await;
        });
        let _ = tokio::try_join!(upload, download);
        Ok(())
    }
}

// ── Listener (QUIC connection lifecycle) ───────────────────────────────

pub(crate) async fn run_hysteria2_listener_with_bound(
    proxy: &Proxy,
    request: Hysteria2InboundRequest,
    bound: crate::protocol_registry::BoundInbound,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let Hysteria2InboundRequest {
        inbound,
        profile,
        up_bps: _up_bps,
        down_bps: _down_bps,
    } = request;
    let listen_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);
    let quic_inbound = match bound {
        crate::protocol_registry::BoundInbound::Quic(e) => e,
        _ => {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 listener requires QUIC transport",
            )))
        }
    };

    let stream_handler = Hysteria2StreamHandler;

    let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

    info!(
        inbound_tag = %inbound.tag,
        protocol = "hysteria2",
        listen = %listen_addr,
        "inbound listener ready"
    );

    loop {
        select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => break,
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
            accept_result = quic_inbound.accept_connection() => {
                match accept_result {
                    Ok(conn) => {
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let profile = profile.clone();
                        let resolver = Arc::clone(&proxy.resolver);
                        let handler = stream_handler.clone();

                        connections.spawn(async move {
                            if let Err(error) = engine.handle_hysteria2_connection(
                                conn, &tag, profile, &handler, resolver,
                            ).await {
                                error!(error = %error, "hysteria2 connection error");
                            }
                            Ok(())
                        });
                    }
                    Err(error) => {
                        error!(error = %error, "hysteria2 accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "hysteria2 connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "hysteria2 connection shutdown error");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "hysteria2", "inbound listener stopped");
    Ok(())
}

impl Proxy {
    /// Handle a single Hysteria2 QUIC connection.
    async fn handle_hysteria2_connection(
        &self,
        conn: quinn::Connection,
        inbound_tag: &str,
        profile: Hysteria2InboundProfile,
        stream_handler: &Hysteria2StreamHandler,
        resolver: Arc<zero_dns::DnsSystem>,
    ) -> Result<(), EngineError> {
        // Derive salt from TLS keying material
        let mut salt = [0u8; 32];
        if conn
            .export_keying_material(&mut salt, b"hysteria2 auth", &[])
            .is_err()
        {
            return Err(EngineError::Io(io::Error::other("quic key export failed")));
        }

        // Wait for auth stream from client
        let (send, recv) = match conn.accept_bi().await {
            Ok(stream) => stream,
            Err(e) => {
                return Err(EngineError::Io(io::Error::other(format!(
                    "accept auth stream: {e}"
                ))));
            }
        };

        let mut auth_stream = Hysteria2Stream::new(send, recv);

        // Read auth frame
        let mut auth_buf = [0u8; 64];
        let n = AsyncSocket::read(&mut auth_stream, &mut auth_buf)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("read auth: {e}"))))?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "hysteria2: EOF on auth stream",
            )));
        }

        if profile.authenticate_client(&salt, &auth_buf[..n]).is_err() {
            let err_resp = profile.auth_error_response("authentication failed");
            let _ = AsyncSocket::write_all(&mut auth_stream, &err_resp).await;
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "hysteria2: auth failed",
            )));
        }

        let ok_resp = profile.auth_ok_response();
        AsyncSocket::write_all(&mut auth_stream, &ok_resp)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("write auth ok: {e}"))))?;

        drop(auth_stream);

        info!(inbound_tag, "hysteria2 auth success");

        // Open local UDP socket for datagram forwarding
        let udp_socket = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                warn!(error = %e, "hysteria2: failed to bind UDP socket, datagrams disabled");
                None
            }
        };

        let mut stream_tasks = JoinSet::new();
        let conn = Arc::new(conn);

        // Spawn datagram reader task
        if let Some(ref udp) = udp_socket {
            let conn_dg = conn.clone();
            let udp_dg = udp.clone();
            let tag = inbound_tag.to_owned();
            let engine_for_h2 = self.clone();
            stream_tasks.spawn(async move {
                Self::hysteria2_datagram_loop(conn_dg, udp_dg, tag, resolver, engine_for_h2).await
            });
        }

        loop {
            select! {
                bi = conn.accept_bi() => {
                    match bi {
                        Ok((send, recv)) => {
                            let engine = self.clone();
                            let tag = inbound_tag.to_owned();
                            let handler = stream_handler.clone();
                            stream_tasks.spawn(async move {
                                // Inline accept: parse connect header, build session
                                let mut stream = Hysteria2Stream::new(send, recv);
                                let mut header_buf = [0u8; 512];
                                let n = match AsyncSocket::read(&mut stream, &mut header_buf).await {
                                    Ok(0) => return Ok(()),
                                    Ok(n) => n,
                                    Err(_) => return Ok(()),
                                };

                                let session = match Hysteria2Inbound
                                    .accept_tcp_connect_header(&header_buf[..n])
                                {
                                    Ok(session) => session,
                                    Err(_) => return Ok(()),
                                };

                                let _ = serve_inbound(
                                    &engine, session, stream, &handler, &tag, None,
                                ).await;
                                Ok(())
                            });
                        }
                        Err(e) => {
                            warn!(error = %e, "hysteria2 accept_bi error");
                            break;
                        }
                    }
                }
                result = stream_tasks.join_next(), if !stream_tasks.is_empty() => {
                    match result {
                        Some(Err(e)) if !e.is_cancelled() => {
                            error!(error = %e, "hysteria2 stream task panicked");
                        }
                        _ => {}
                    }
                }
            }
        }

        stream_tasks.abort_all();
        Ok(())
    }

    /// Datagram forwarding loop (unchanged).
    async fn hysteria2_datagram_loop(
        conn: Arc<quinn::Connection>,
        _udp_socket: Arc<tokio::net::UdpSocket>,
        inbound_tag: String,
        _resolver: Arc<zero_dns::DnsSystem>,
        proxy: Proxy,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(&inbound_tag).await?;
        let mut udp_session = hysteria2::Hysteria2InboundUdpSession::new();

        let mut direct_buf = [0u8; 65536];

        loop {
            let (direct_sock, chain_tasks) = dispatch.poll_sockets();

            select! {
                dg = conn.read_datagram() => {
                    match dg {
                        Ok(data) => {
                            if let Ok(request) = udp_session.decode_request(&data) {
                                let _ = UdpPipe::new(&proxy, &mut dispatch)
                                    .dispatch(UdpPipeInput {
                                        target: request.target().clone(),
                                        port: request.port(),
                                        payload: request.payload(),
                                        protocol: ProtocolType::Hysteria2,
                                        auth: None,
                                        client_session_id: None,
                                    })
                                    .await.inspect(|sid| {
                                    udp_session.record_proxy_session(*sid, &request);
                                }).inspect_err(|e| {
                                    warn!(error = %e, "h2 udp dispatch failed");
                                });
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "hysteria2 read_datagram error");
                            break Ok(());
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                        let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                        let target = match ip {
                            zero_traits::IpAddress::V4(b) => Address::Ipv4(b),
                            zero_traits::IpAddress::V6(b) => Address::Ipv6(b),
                        };
                        let _ = udp_session.send_response(
                            &conn,
                            sid,
                            &target,
                            sender.port(),
                            &direct_buf[..n],
                        );
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                let _ = udp_session.send_response(&conn, sid, &target, port, &payload);
                            }
                        }
                        Ok(Err(error)) => warn!(error = %error, "h2 chain response error"),
                        Err(e) => warn!(error = %e, "h2 chain task panicked"),
                    }
                }
            }
        }
    }
}
