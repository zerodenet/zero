//! Mieru inbound encrypted handshake and AEAD-framed relay.

use std::net::SocketAddr;

use async_trait::async_trait;
use mieru::MieruInbound;
use tokio::io::AsyncReadExt;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::DnsResolver;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

type MieruClientStream = mieru::MieruInboundStream<TcpRelayStream>;

#[derive(Debug)]
pub(crate) struct MieruInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) users: Vec<(String, String)>,
}

// Handler.

#[derive(Clone)]
pub(crate) struct MieruInboundHandler {
    mieru_inbound: MieruInbound,
    users: Vec<(String, String)>,
}

#[async_trait]
impl InboundProtocol for MieruInboundHandler {
    type ClientStream = MieruClientStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = crate::transport::MeteredStream::new(stream);
        let accept = self
            .mieru_inbound
            .accept_request(&mut metered, &self.users)
            .await?;

        let mut client = mieru::MieruInboundStream::new(metered.into_inner(), accept);

        let mut session = client.accept_tunneled_socks5_session().await?;
        let mut sa = zero_core::SessionAuth::new("mieru");
        sa.principal_key = Some("mieru".to_owned());
        session.apply_auth(sa);

        Ok((session, client))
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Mieru handshake already confirms success
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        // Mieru protocol has no explicit blocked response;
        // the connection close serves as the signal.
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.send_blocked(_client).await
    }
}

// Listener.

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    request: MieruInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MieruInboundRequest { inbound, users } = request;
    let local_addr = listener.local_addr()?;

    let handler = MieruInboundHandler {
        mieru_inbound: MieruInbound,
        users,
    };

    let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

    info!(
        inbound_tag = %inbound.tag,
        protocol = "mieru",
        listen = %local_addr,
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
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, remote_addr)) => {
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let handler = handler.clone();
                        let source_addr = remote_addr_to_socket(remote_addr);
                        connections.spawn(async move {
                            match handler.accept(stream.into()).await {
                                Ok((session, client)) => {
                                    if session.network == zero_core::Network::Udp {
                                        let _ = engine.run_mieru_udp_relay(
                                            client, &session, &tag,
                                        ).await;
                                    } else {
                                        let _ = serve_inbound(
                                            &engine, session, client, &handler,
                                            &tag, source_addr,
                                        ).await;
                                    }
                                }
                                Err(error) => {
                                    log_listener_connection_error(
                                        "mieru", &tag, &remote_addr, &error,
                                    );
                                }
                            }
                            Ok(())
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "mieru: accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                match result {
                    Some(Err(error)) if !error.is_cancelled() => {
                        error!(error = %error, "mieru connection task panicked");
                    }
                    _ => {}
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "mieru shutdown error");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "mieru", "listener stopped");
    Ok(())
}

// UDP relay.

impl Proxy {
    /// Run a Mieru UDP relay: read encrypted data segments, decrypt,
    /// unwrap Mieru UDP associate framing, parse SOCKS5 UDP packet,
    /// forward to target, and send responses back.
    async fn run_mieru_udp_relay(
        &self,
        mut client: MieruClientStream,
        _session: &Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let udp_socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru udp bind: {e}"))))?;

        let mut read_buf = [0u8; 65536];
        let mut recv_buf = [0u8; 65536];
        let mut udp_session = mieru::MieruInbound.udp_session();

        loop {
            tokio::select! {
                // Read decrypted data from Mieru client
                read = client.read(&mut read_buf) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = &read_buf[..n];
                            if let Ok(request) = udp_session.decode_request(data) {
                                let target_addr = if let Some(addr) = request.target_socket_addr() {
                                    Some(addr)
                                } else if let Some((domain, _port)) = request.target_domain() {
                                    match self.resolver.resolve(domain).await {
                                        Ok(ips) => ips
                                            .first()
                                            .copied()
                                            .map(|ip| request.resolved_target_socket_addr(ip)),
                                        Err(_) => None,
                                    }
                                } else {
                                    None
                                };

                                if let Some(addr) = target_addr {
                                    udp_session.record_request_target(addr, &request);
                                    let payload = request.into_payload();
                                    let _ = udp_socket.send_to(&payload, addr).await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "mieru udp read error");
                            break;
                        }
                    }
                }
                // Read responses from UDP socket
                recv = udp_socket.recv_from(&mut recv_buf) => {
                    match recv {
                        Ok((n, sender)) => {
                            if let Err(e) = udp_session
                                .write_response_tokio(&mut client, sender, &recv_buf[..n])
                                .await
                            {
                                tracing::warn!(
                                    error = %e, "mieru udp write error"
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "mieru udp recv_from error");
                            break;
                        }
                    }
                }
            }
        }

        tracing::info!(inbound_tag = %inbound_tag, "mieru udp relay stopped");
        Ok(())
    }
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
