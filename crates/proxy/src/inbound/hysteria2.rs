//! Hysteria2 inbound: QUIC accept, HMAC auth, TCP stream dispatch.
//!
//! TCP stream relay uses the `InboundProtocol` trait with a custom relay
//! that handles QUIC stream I/O (not raw TCP).

use async_trait::async_trait;
use hysteria2::{Hysteria2Inbound, Hysteria2InboundProfile};
use std::io;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::{
    record_udp_inbound_response_rx, record_udp_inbound_response_tx, udp_response_session_id,
    wait_for_upstream_idle,
};
use crate::runtime::Proxy;
use crate::transport::{copy_one_way, Hysteria2Stream};

#[derive(Debug)]
pub(crate) struct Hysteria2InboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: Hysteria2InboundProfile,
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
    let Hysteria2InboundRequest { inbound, profile } = request;
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
                        let handler = stream_handler.clone();

                        connections.spawn(async move {
                            if let Err(error) = engine.handle_hysteria2_connection(
                                conn, &tag, profile, &handler,
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
    ) -> Result<(), EngineError> {
        // Wait for auth stream from client.
        let (send, recv) = match conn.accept_bi().await {
            Ok(stream) => stream,
            Err(e) => {
                return Err(EngineError::Io(io::Error::other(format!(
                    "accept auth stream: {e}"
                ))));
            }
        };

        let mut auth_stream = Hysteria2Stream::new(send, recv);

        profile
            .authenticate_quic_connection(&conn, &mut auth_stream)
            .await?;
        drop(auth_stream);

        info!(inbound_tag, "hysteria2 auth success");

        let mut stream_tasks = JoinSet::new();
        let conn = std::sync::Arc::new(conn);

        let conn_dg = conn.clone();
        let tag = inbound_tag.to_owned();
        let engine_for_h2 = self.clone();
        stream_tasks
            .spawn(async move { Self::hysteria2_datagram_loop(conn_dg, tag, engine_for_h2).await });

        loop {
            select! {
                bi = conn.accept_bi() => {
                    match bi {
                        Ok((send, recv)) => {
                            let engine = self.clone();
                            let tag = inbound_tag.to_owned();
                            let handler = stream_handler.clone();
                            stream_tasks.spawn(async move {
                                let mut stream = Hysteria2Stream::new(send, recv);
                                let session = match Hysteria2Inbound.accept_tcp_stream(&mut stream).await {
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
        conn: std::sync::Arc<quinn::Connection>,
        inbound_tag: String,
        proxy: Proxy,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(&inbound_tag).await?;
        let mut udp_session = hysteria2::Hysteria2Inbound.udp_session();

        let mut direct_buf = [0u8; 65536];
        let mut upstream_buf = [0u8; 65536];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                dg = conn.read_datagram() => {
                    match dg {
                        Ok(data) => {
                            if let Ok(request) = udp_session.decode_request(&data) {
                                let request = request.into_dispatch_parts();
                                let _ = UdpPipe::new(&proxy, &mut dispatch)
                                    .dispatch(UdpPipeInput {
                                        target: request.target,
                                        port: request.port,
                                        payload: &request.payload,
                                        protocol: ProtocolType::Hysteria2,
                                        auth: None,
                                        client_session_id: request.client_session_id,
                                    })
                                    .await.inspect(|sid| {
                                    udp_session.record_proxy_session(*sid, request.request_session_id);
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
                    let session_id = dispatch.direct_response_session_id(sender);
                    if let Some(sid) = session_id {
                        record_udp_inbound_response_rx(&proxy, session_id, n);
                        let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                        if let Ok(Some(written)) = udp_session.send_response_to_ip(
                            &conn,
                            sid,
                            ip,
                            sender.port(),
                            &direct_buf[..n],
                        ) {
                            record_udp_inbound_response_tx(&proxy, session_id, written);
                        }
                    }
                }

                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            proxy.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(proxy.udp_upstream_idle_timeout());
                            let (target, port, payload) = pkt.into_parts();
                            let session_id = udp_response_session_id(&dispatch, &target, port);
                            if let Some(sid) = session_id {
                                record_udp_inbound_response_rx(&proxy, session_id, payload.len());
                                if let Ok(Some(written)) = udp_session.send_response(&conn, sid, &target, port, &payload) {
                                    record_udp_inbound_response_tx(&proxy, session_id, written);
                                }
                            }
                        }
                        Err(error) => warn!(error = %error, "h2 upstream response error"),
                    }
                }

                _ = wait_for_upstream_idle(socks5_idle) => {}

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                record_udp_inbound_response_rx(&proxy, session_id, payload.len());
                                if let Ok(Some(written)) = udp_session.send_response(&conn, sid, &target, port, &payload) {
                                    record_udp_inbound_response_tx(&proxy, session_id, written);
                                }
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
