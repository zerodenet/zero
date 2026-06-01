//! TUN inbound — virtual network interface.
//!
//! Reads raw IP packets from a [`TunDevice`], feeds them to a
//! [`NetworkStack`] (which handles TCP termination and UDP forwarding),
//! and dispatches established TCP connections through `serve_inbound()`.
//!
//! The stack is pluggable: `UserNetworkStack` (default), or future
//! `SystemStack` / `MixedStack` — the inbound handler only depends on
//! [`TcpStack`] / [`UdpStack`] traits.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::interval;
use tracing::{error, info, warn};

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_stack::{UserNetworkStack, UserTcpStream};
use zero_traits::{NetworkStack, SocketAddress as TraitsSocketAddr, TcpStack, UdpStack};
use zero_tun::TunDevice;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::{Proxy, TunInfo};
use crate::transport::TcpRelayStream;

// ── Protocol handler ──────────────────────────────────────────────────

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
    async fn send_upstream_failure(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── Dispatch loop ─────────────────────────────────────────────────────

/// How often to clean up idle UDP relay tasks.
const UDP_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
/// Idle timeout for UDP relay tasks.
const UDP_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

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
    let mut cleanup_tick = interval(UDP_CLEANUP_INTERVAL);

    // UDP relay: local socket for sending/receiving datagrams to destinations.
    let relay_sock = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "tun udp relay socket bind failed");
            return;
        }
    };
    // Track pending UDP requests: (src, dst) → last_active for response matching.
    let pending = Mutex::new(HashMap::<(TraitsSocketAddr, TraitsSocketAddr), Instant>::new());

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

            // ── UDP datagram from stack → forward to destination ──
            Some((n, src, dst)) = udp.recv_from(&mut udp_buf) => {
                let target = sockaddr_to_std(&dst);
                if let Err(e) = relay_sock.send_to(&udp_buf[..n], target).await {
                    warn!(error = %e, %target, "tun udp send_to failed");
                } else {
                    pending.lock().await.insert((src, dst), Instant::now());
                }
            }

            // ── Periodic cleanup ──
            _ = cleanup_tick.tick() => {
                let mut pend = pending.lock().await;
                pend.retain(|_, last| last.elapsed() < UDP_IDLE_TIMEOUT);
            }

            // ── Raw packet from TUN device ──
            r = async {
                let mut dev = device.lock().await;
                dev.read(&mut buf).await
            } => {
                match r {
                    Ok(0) => break,
                    Ok(n) => {
                        tcp.feed(&buf[..n]).await;
                        udp.feed(&buf[..n]).await;
                        // After feeding, poll for UDP responses.
                        poll_udp_responses(&relay_sock, udp, &pending).await;
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

/// Non-blocking poll for UDP responses to pending TUN requests.
///
/// Tries to receive from the relay socket.  When a datagram arrives,
/// looks up the sender's address in the pending map to determine the
/// original TUN-side source; if matched, sends the response back
/// through the UDP stack.
async fn poll_udp_responses(
    sock: &UdpSocket,
    udp: &impl UdpStack,
    pending: &Mutex<HashMap<(TraitsSocketAddr, TraitsSocketAddr), Instant>>,
) {
    let mut resp_buf = [0u8; 65536];
    match sock.try_recv_from(&mut resp_buf) {
        Ok((n, from)) => {
            let mut pend = pending.lock().await;
            let key = pend
                .iter()
                .find(|((_src, dst), _)| sockaddr_to_std(dst) == from)
                .map(|((src, dst), _)| (*src, *dst));

            if let Some((src, dst)) = key {
                udp.send_to(&resp_buf[..n], dst, src).await;
                pend.insert((src, dst), Instant::now());
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            // Nothing available — expected.
        }
        Err(e) => {
            warn!(error = %e, "tun udp recv error");
        }
    }
}

// ── Address conversion helpers ────────────────────────────────────────

fn sockaddr_to_std(sa: &TraitsSocketAddr) -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let ip: IpAddr = match sa.ip {
        zero_traits::IpAddress::V4(o) => IpAddr::V4(Ipv4Addr::from(o)),
        zero_traits::IpAddress::V6(o) => IpAddr::V6(Ipv6Addr::from(o)),
    };
    SocketAddr::new(ip, sa.port)
}

fn sockaddr_to_address(sa: &TraitsSocketAddr) -> Address {
    match sa.ip {
        zero_traits::IpAddress::V4(o) => Address::Ipv4(o),
        zero_traits::IpAddress::V6(o) => Address::Ipv6(o),
    }
}

// ── Proxy entry points ────────────────────────────────────────────────

impl Proxy {
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

        // Writer task.
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
