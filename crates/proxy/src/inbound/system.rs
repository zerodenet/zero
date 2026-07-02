//! System-level traffic interception inbound.
//!
//! Unlike the TUN inbound (which reads raw IP packets from a virtual
//! interface), the system inbound uses OS-level traffic redirection:
//!
//! | Platform | Redirection mechanism            |
//! |----------|----------------------------------|
//! | Linux    | iptables -t nat REDIRECT         |
//! | macOS    | pf.conf rdr rule                 |
//! | Windows  | WFP ALE connect redirect (built-in, no driver) |
//!
//! The redirected traffic arrives at a local TCP listener managed by
//! [`SystemTcpStack`].  Each connection is dispatched through the same
//! `serve_inbound()` pipeline as any other inbound protocol.

use std::io;
use std::net::SocketAddr;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tracing::info;

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_stack::SystemTcpStack;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_system_tcp_stack_loop, SystemTcpStackLoopRequest};
use crate::runtime::Proxy;

// ── Protocol handler ──────────────────────────────────────────────────

struct SystemProtocol;

#[async_trait]
impl InboundProtocol for SystemProtocol {
    type ClientStream = TcpStream;

    async fn accept(
        &self,
        _: crate::transport::TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "system accept handled by stack",
        )))
    }
    async fn send_ok(&self, _: &mut TcpStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_blocked(&self, _: &mut TcpStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_upstream_failure(&self, _: &mut TcpStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── System inbound loop ───────────────────────────────────────────────

async fn system_tcp_loop(
    proxy: Proxy,
    stack: SystemTcpStack,
    tag: String,
    shutdown: watch::Receiver<bool>,
) {
    run_system_tcp_stack_loop(SystemTcpStackLoopRequest {
        proxy: &proxy,
        inbound_tag: tag,
        stack,
        shutdown,
        handler: |proxy: Proxy,
                  tag: String,
                  stream: TcpStream,
                  source: zero_traits::SocketAddress,
                  destination: zero_traits::SocketAddress| async move {
            let session = Session::new(
                0,
                sockaddr_to_address(&destination),
                destination.port,
                Network::Tcp,
                ProtocolType::Unknown,
            );
            let source_addr = sockaddr_to_std(&source);
            let _ = serve_inbound(
                &proxy,
                session,
                stream,
                &SystemProtocol,
                &tag,
                Some(source_addr),
            )
            .await;
        },
    })
    .await;
}

// ── Address helpers ────────────────────────────────────────────────────

fn sockaddr_to_std(sa: &zero_traits::SocketAddress) -> SocketAddr {
    zero_platform_tokio::socket_address_to_socket_addr(*sa)
}

fn sockaddr_to_address(sa: &zero_traits::SocketAddress) -> Address {
    match sa.ip {
        zero_traits::IpAddress::V4(o) => Address::Ipv4(o),
        zero_traits::IpAddress::V6(o) => Address::Ipv6(o),
    }
}

// ── Proxy entry points ────────────────────────────────────────────────

impl Proxy {
    /// Start system-level traffic interception.
    ///
    /// Creates a TCP listener on `listen_addr` that receives traffic
    /// redirected by OS-level mechanisms (iptables/pf/WFP).
    pub async fn start_system_inbound(
        &self,
        listen_addr: SocketAddr,
        tag: &str,
    ) -> Result<(), EngineError> {
        let stack = SystemTcpStack::bind(listen_addr)
            .await
            .map_err(EngineError::Io)?;

        let actual = stack.local_addr().map_err(EngineError::Io)?;
        info!(inbound_tag = tag, listen = %actual, "system inbound ready");

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let proxy = self.clone();
        let t = tag.to_owned();
        tokio::spawn(async move {
            system_tcp_loop(proxy, stack, t, shutdown_rx).await;
        });

        Ok(())
    }
}
