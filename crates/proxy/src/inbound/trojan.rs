//! Trojan inbound: TLS accept, protocol auth, route, TCP/UDP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use trojan::TrojanInbound;
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{log_completed_udp_flow, wait_for_upstream_idle};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[derive(Debug)]
pub(crate) struct TrojanInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) password: String,
    pub(crate) tls: Option<zero_config::TlsConfig>,
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
    password: String,
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
        let accept = self
            .trojan_inbound
            .accept(&mut sock, std::slice::from_ref(&self.password))
            .await?;
        let mut session: Session = accept.session;
        let mut sa = zero_core::SessionAuth::new("trojan");
        sa.principal_key = Some(self.password.clone());
        session.apply_auth(sa);
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
        password,
        tls: tls_cfg,
    } = request;
    let tls_cfg = tls_cfg.ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trojan requires TLS",
        ))
    })?;
    let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, proxy.config.source_dir())?;
    let tag = inbound.tag.clone();

    let handler = TrojanInboundHandler {
        trojan_inbound: TrojanInbound,
        password,
        tls_acceptor: acceptor,
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
                let source_addr = remote_addr_to_socket(peer);
                conns.spawn(async move {
                    match h.accept(s.into()).await {
                        Ok((session, client)) => {
                            let result = if session.network == zero_core::Network::Udp {
                                p.run_trojan_udp_relay(client, session, &t).await
                            } else {
                                serve_inbound(&p, session, client, &h, &t, source_addr).await
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
                packet = trojan::read_udp_flow_packet(&mut client) => {
                    match packet {
                        Ok(packet) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput {
                                    target: packet.target,
                                    port: packet.port,
                                    payload: &packet.payload,
                                    protocol: zero_core::ProtocolType::Trojan,
                                    auth: auth.as_ref(),
                                    client_session_id: None,
                                })
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

                    let target = match zero_platform_tokio::socket_addr_to_ip(sender) {
                        zero_traits::IpAddress::V4(bytes) => zero_core::Address::Ipv4(bytes),
                        zero_traits::IpAddress::V6(bytes) => zero_core::Address::Ipv6(bytes),
                    };
                    let session_id = dispatch.direct_response_session_id(sender);
                    if let Some(sid) = session_id {
                        self.record_session_outbound_rx(sid, n as u64);
                    }
                    trojan::write_udp_flow_packet(&mut client, &target, sender.port(), &direct_buf[..n]).await?;
                    if let Some(sid) = session_id {
                        self.record_session_inbound_tx(sid, n as u64);
                    }
                }
                upstream = upstream_udp.recv_packet(&mut upstream_buf) => {
                    match upstream {
                        Ok(read) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(self.udp_upstream_idle_timeout());
                            if let Ok(pkt) = socks5::decode_udp_associate_response(&upstream_buf[..read]) {
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                    self.record_session_outbound_rx(sid, pkt.payload.len() as u64);
                                }
                                let target = pkt.target;
                                let port = pkt.port;
                                let payload = pkt.payload;
                                trojan::write_udp_flow_packet(&mut client, &target, port, &payload).await?;
                                if let Some(sid) = dispatch.session_id_by_target(&target, port, None) {
                                    self.record_session_inbound_tx(sid, payload.len() as u64);
                                }
                            }
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
                            if let Some(sid) = session_id {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            let payload_len = payload.len() as u64;
                            trojan::write_udp_flow_packet(&mut client, &target, port, &payload).await?;
                            if let Some(sid) = session_id {
                                self.record_session_inbound_tx(sid, payload_len);
                            }
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

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
