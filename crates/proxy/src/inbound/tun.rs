//! TUN inbound — virtual network interface.
//!
//! Reads raw IP packets from a [`TunDevice`], feeds them to a
//! [`NetworkStack`] (which handles TCP termination and UDP forwarding),
//! and dispatches established TCP connections through the kernel's
//! `serve_inbound()` pipeline.
//!
//! The stack is pluggable: `UserNetworkStack` (default, user-space TCP
//! state machine), or future `SystemStack` / `MixedStack` — the inbound
//! handler only depends on the [`TcpStack`] / [`UdpStack`] traits.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::{mpsc, watch, Mutex};
use tracing::{error, info, warn};

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_stack::{UserNetworkStack, UserTcpStream};
use zero_traits::{NetworkStack, TcpStack, UdpStack};
use zero_tun::TunDevice;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::{Proxy, TunInfo};
use crate::transport::TcpRelayStream;

// ── Protocol handler ──────────────────────────────────────────────────

/// Minimal `InboundProtocol` implementation for TUN connections.
///
/// TUN doesn't have a protocol-level handshake — the TCP stack
/// terminates the handshake before the connection reaches us.
/// All response methods are no-ops.
struct TunProtocol;

#[async_trait]
impl InboundProtocol for TunProtocol {
    type ClientStream = UserTcpStream;

    async fn accept(
        &self,
        _: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "tun accept handled by stack",
        )))
    }

    async fn send_ok(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_blocked(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }
    async fn send_upstream_failure(
        &self,
        _: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── Dispatch loop ─────────────────────────────────────────────────────

/// Main TUN dispatch loop.
///
/// Reads raw packets from the device, feeds them to the stack,
/// and multiplexes between:
/// - TCP connections arriving via `stack.accept()`
/// - UDP datagrams arriving via `stack.recv_from()`
async fn tun_loop<S: NetworkStack + Send + Sync + 'static>(
    proxy: Proxy,
    device: Arc<Mutex<impl TunDevice + 'static>>,
    stack: S,
    tag: String,
    mut shutdown: watch::Receiver<bool>,
) where
    S::Tcp: TcpStack<Connection = UserTcpStream>,
{
    let tcp = stack.tcp();
    let udp = stack.udp();
    let mut buf = vec![0u8; 65536];
    let mut udp_buf = vec![0u8; 65536];

    loop {
        tokio::select! {
            biased;

            // ── Shutdown signal ──
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("tun shutdown requested");
                    break;
                }
                continue;
            }

            // ── Established TCP connection from stack ──
            Some((stream, src, dst)) = tcp.accept() => {
                let src_addr = sockaddr_to_std(&src);
                let session = Session::new(
                    0,
                    sockaddr_to_address(&dst),
                    dst.port,
                    Network::Tcp,
                    ProtocolType::Unknown,
                );
                let p = proxy.clone();
                let t = tag.clone();
                tokio::spawn(async move {
                    let _ = serve_inbound(
                        &p, session, stream, &TunProtocol, &t, Some(src_addr),
                    ).await;
                });
            }

            // ── UDP datagram from stack ──
            Some((n, _src, dst)) = udp.recv_from(&mut udp_buf) => {
                // UDP is forwarded by the proxy's UDP subsystem.
                // For now, log and drop — full UDP relay through the
                // proxy pipeline is a separate concern (see UDP ASSOCIATE
                // in SOCKS5, VLESS UoT, Hysteria2 QUIC).
                let _ = (n, dst);
            }

            // ── Raw packet from TUN device ──
            r = async {
                let mut dev = device.lock().await;
                dev.read(&mut buf).await
            } => {
                match r {
                    Ok(0) => break,
                    Ok(n) => {
                        // Feed to both stacks — each ignores non-matching protocol.
                        tcp.feed(&buf[..n]).await;
                        udp.feed(&buf[..n]).await;
                    }
                    Err(e) => {
                        error!(error = %e, "tun read");
                        break;
                    }
                }
            }
        }
    }
}

// ── Address conversion helpers ────────────────────────────────────────

fn sockaddr_to_std(sa: &zero_traits::SocketAddress) -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let ip: IpAddr = match sa.ip {
        zero_traits::IpAddress::V4(o) => IpAddr::V4(Ipv4Addr::from(o)),
        zero_traits::IpAddress::V6(o) => IpAddr::V6(Ipv6Addr::from(o)),
    };
    SocketAddr::new(ip, sa.port)
}

fn sockaddr_to_address(sa: &zero_traits::SocketAddress) -> Address {
    match sa.ip {
        zero_traits::IpAddress::V4(o) => Address::Ipv4(o),
        zero_traits::IpAddress::V6(o) => Address::Ipv6(o),
    }
}

// ── Proxy entry points ────────────────────────────────────────────────

impl Proxy {
    /// Start the TUN device using the user-space network stack.
    pub async fn start_tun(
        &self,
        name: Option<&str>,
        addr: &str,
        _mask: &str,
        _mtu: u16,
        tag: &str,
    ) -> Result<(), EngineError> {
        {
            let info = self.tun_info.lock().unwrap();
            if info.is_some() {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "TUN is already running",
                )));
            }
        }

        let device = zero_tun::create(name).map_err(EngineError::Io)?;
        let dn = device.name().to_owned();
        info!(inbound_tag = tag, name = %dn, addr = %addr, "tun device created");

        let device = Arc::new(Mutex::new(device));

        // Outbound packet channel: stack → writer task → TUN device.
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Vec<u8>>(256);

        // Writer task: drains outbound packets from the stack and writes
        // them to the TUN device.
        let writer_dev = device.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            while let Some(pkt) = outbound_rx.recv().await {
                let mut dev = writer_dev.lock().await;
                if let Err(e) = dev.write_all(&pkt).await {
                    warn!(error = %e, "tun write failed");
                }
            }
        });

        // Create user-space network stack.
        let stack = UserNetworkStack::new(outbound_tx, 1500);

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        *self.tun_shutdown.lock().unwrap() = Some(shutdown_tx);
        *self.tun_info.lock().unwrap() = Some(TunInfo {
            name: dn,
            addr: addr.to_owned(),
            tag: tag.to_owned(),
        });

        let proxy = self.clone();
        let t = tag.to_owned();
        tokio::spawn(async move {
            tun_loop(proxy, device, stack, t, shutdown_rx).await;
        });

        Ok(())
    }

    pub fn stop_tun(&self) -> Result<(), EngineError> {
        let mut s = self.tun_shutdown.lock().unwrap();
        if let Some(tx) = s.take() {
            let _ = tx.send(true);
            *self.tun_info.lock().unwrap() = None;
            Ok(())
        } else {
            Err(EngineError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "TUN is not running",
            )))
        }
    }

    #[allow(dead_code)]
    pub(crate) fn tun_status(&self) -> Option<TunInfo> {
        self.tun_info.lock().unwrap().clone()
    }
}
