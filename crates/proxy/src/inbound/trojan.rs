//! Trojan inbound: TLS accept, protocol auth, route, TCP/UDP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use trojan::{TrojanInbound, TrojanInboundProfile};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_received,
    record_direct_udp_response_received, record_upstream_udp_response_received,
    wait_for_upstream_idle,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) struct TrojanInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: TrojanInboundProfile,
    pub(crate) tls_acceptor: crate::transport::TlsAcceptor,
}

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.
struct TlsStream(tokio_rustls::server::TlsStream<TcpRelayStream>);

impl AsyncSocket for TlsStream {
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        tokio::io::AsyncReadExt::read(&mut self.0, buf).await
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(&mut self.0, buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::shutdown(&mut self.0).await
    }
}

// Trait-based handler.

#[derive(Clone)]
pub(crate) struct TrojanInboundHandler {
    trojan_inbound: TrojanInbound,
    profile: TrojanInboundProfile,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for TrojanInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // TLS accept
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(e)))?;
        // Trojan protocol auth
        let mut sock = TlsStream(tls);
        let accept = self.profile.accept(self.trojan_inbound, &mut sock).await?;
        let mut session: Session = accept.session;
        session.apply_auth(self.profile.inbound_auth());
        Ok((session, TcpRelayStream::new(sock.0)))
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no success response
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no blocked response; just close.
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    // relay uses default
}

// Listener.

pub(crate) async fn run_trojan_listener_with_bound(
    proxy: &Proxy,
    request: TrojanInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let TrojanInboundRequest {
        inbound,
        profile,
        tls_acceptor,
    } = request;
    let tag = inbound.tag.clone();

    let handler = TrojanInboundHandler {
        trojan_inbound: TrojanInbound,
        profile,
        tls_acceptor,
    };

    info!(inbound_tag = %tag, protocol = "trojan", listen = %listener.local_addr()?, "started");

    let mut conns = JoinSet::new();
    loop {
        select! {
            _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
            r = listener.accept() => {
                let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                let p = proxy.clone();
                let t = tag.clone();
                let h = handler.clone();
                let source_addr = zero_platform_tokio::remote_ip_to_socket_addr(peer);
                conns.spawn(async move {
                    match h.accept(s.into()).await {
                        Ok((session, client)) => {
                            let result = match trojan::classify_inbound_session(&session) {
                                trojan::TrojanInboundSessionKind::Udp => {
                                    p.run_trojan_udp_relay(client, session, &t).await
                                }
                                trojan::TrojanInboundSessionKind::Tcp => {
                                    serve_inbound(&p, session, client, &h, &t, source_addr).await
                                }
                            };
                            if let Err(e) = result {
                                if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                    io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                                { warn!(?source_addr, %e, "trojan failed"); }
                            }
                        }
                        Err(e) => {
                            if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                            { warn!(?source_addr, %e, "trojan auth failed"); }
                        }
                    }
                });
            }
            r = conns.join_next(), if !conns.is_empty() => {
                if let Some(Err(e)) = r { if !e.is_cancelled() { error!(%e, "task panicked"); } }
            }
        }
    }
    conns.abort_all();
    info!(inbound_tag = %tag, "stopped");
    Ok(())
}

impl Proxy {
    async fn run_trojan_udp_relay(
        &self,
        mut client: TcpRelayStream,
        session: Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();
        let udp_session = trojan::TrojanInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "trojan_udp",
            "trojan udp session started"
        );

        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "trojan_udp",
                        "trojan udp session idle timeout"
                    );
                    break;
                }
                packet = udp_session.read_inbound_dispatch(&mut client) => {
                    match packet {
                        Ok(inbound_dispatch) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput::from_inbound_dispatch(
                                    &inbound_dispatch,
                                    auth.as_ref(),
                                ))
                                .await
                            {
                                warn!(error = %error, "failed to process trojan udp packet");
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "trojan udp client read error");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let response_accounting =
                        record_direct_udp_response_received(self, &dispatch, sender, n);
                    let written = udp_session
                        .write_response_to_socket_addr_tokio(&mut client, sender, &direct_buf[..n])
                        .await?;
                    response_accounting.record_sent(written);
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            let response = record_upstream_udp_response_received(
                                self,
                                &mut dispatch,
                                self.udp_upstream_idle_timeout(),
                                pkt,
                            );
                            let written = udp_session.write_response(
                                &mut client,
                                &response.target,
                                response.port,
                                &response.payload,
                            ).await?;
                            response.accounting.record_sent(written);
                        }
                        Err(error) => {
                            warn!(error = %error, "trojan udp socks5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            let response_accounting =
                                record_chain_udp_response_received(self, session_id, payload.len());
                            let written = udp_session.write_response(&mut client, &target, port, &payload).await?;
                            response_accounting.record_sent(written);
                        }
                        Ok(Err(error)) => warn!(error = %error, "trojan udp chain response error"),
                        Err(error) => warn!(error = %error, "trojan udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "trojan_udp",
            "trojan udp session ended"
        );

        Ok(())
    }
}
