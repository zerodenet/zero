//! Direct inbound — fixed-target forwarder.
//!
//! Listens on a port, accepts raw TCP connections with no protocol
//! handshake, and forwards all traffic through the kernel pipeline
//! to a configured outbound (node or group).

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;

use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

// ── Handler ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct DirectInboundHandler;

#[async_trait]
impl InboundProtocol for DirectInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // Direct inbound has no protocol handshake — the session is built
        // from config in the listener and dispatched via serve_inbound_direct.
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "direct accept handled inline by listener",
        )))
    }

    async fn send_ok(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_blocked(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_upstream_failure(&self, _: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    // relay uses default
}

// ── Listener ────────────────────────────────────────────────────────────

impl Proxy {
    pub(crate) async fn run_direct_listener_with_bound(
        &self,
        inbound: zero_config::InboundConfig,
        listener: zero_platform_tokio::TokioListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (target, port) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Direct { target, port } => (target.clone(), *port),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "direct listener requires direct config",
                )))
            }
        };

        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();
        let handler = DirectInboundHandler;

        info!(
            inbound_tag = %inbound.tag, protocol = "direct",
            target = ?target, port = ?port,
            listen = %local_addr, "inbound listener ready"
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
                            let engine = self.clone();
                            let tag = inbound.tag.clone();
                            let tgt = target.clone();
                            let pt = port;
                            let h = handler.clone();
                            let src = remote_addr_to_socket(remote_addr);
                            connections.spawn(async move {
                                let address = match tgt.as_deref() {
                                    Some(d) if d.parse::<std::net::Ipv4Addr>().is_ok() => {
                                        Address::Ipv4(d.parse::<std::net::Ipv4Addr>().unwrap().octets())
                                    }
                                    Some(d) if d.parse::<std::net::Ipv6Addr>().is_ok() => {
                                        Address::Ipv6(d.parse::<std::net::Ipv6Addr>().unwrap().octets())
                                    }
                                    Some(d) => Address::Domain(d.to_owned()),
                                    None => return,
                                };
                                let session = Session::new(
                                    0, address, pt.unwrap_or(443),
                                    Network::Tcp, ProtocolType::Unknown,
                                );
                                let _ = serve_inbound(
                                    &engine, session, TcpRelayStream::from(stream),
                                    &h, &tag, src,
                                ).await;
                            });
                        }
                        Err(e) => { error!(error = %e, "direct: accept error"); }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(e)) = result {
                        if !e.is_cancelled() { error!(error = %e, "direct connection task panicked"); }
                    }
                }
            }
        }

        connections.abort_all();
        info!(inbound_tag = %inbound.tag, protocol = "direct", listen = %local_addr, "inbound listener stopped");
        Ok(())
    }
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.and_then(|ip| match ip {
        zero_traits::IpAddress::V4(o) => Some(std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::from(o)),
            0,
        )),
        zero_traits::IpAddress::V6(o) => Some(std::net::SocketAddr::new(
            std::net::IpAddr::V6(std::net::Ipv6Addr::from(o)),
            0,
        )),
    })
}
