//! System-level network stack.
//!
//! Unlike [`UserTcpStack`] (which terminates TCP in user space from raw IP
//! packets), the system stack delegates TCP termination and routing to the
//! operating system.  Traffic is redirected to a local listener via OS-level
//! mechanisms:
//!
//! | Platform | Mechanism                    |
//! |----------|------------------------------|
//! | Linux    | iptables/nftables REDIRECT   |
//! | macOS    | pf redirect                  |
//! | Windows  | WFP ALE connect redirect     |
//!
//! The stack presents connections via [`TcpStack::accept`] — the same trait
//! that [`UserTcpStack`] implements, making the two interchangeable.

use std::io;
use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tracing::warn;

use zero_traits::{SocketAddress, TcpStack, UdpStack};

// ── Address helpers ────────────────────────────────────────────────────

fn std_to_sockaddr(addr: SocketAddr) -> SocketAddress {
    use std::net::IpAddr;
    let ip = match addr.ip() {
        IpAddr::V4(v4) => zero_traits::IpAddress::V4(v4.octets()),
        IpAddr::V6(v6) => zero_traits::IpAddress::V6(v6.octets()),
    };
    SocketAddress::new(ip, addr.port())
}

// ── SystemTcpStack ─────────────────────────────────────────────────────

/// TCP stack backed by an OS-level traffic redirect.
///
/// Creates a local TCP listener.  The caller (or the platform's traffic
/// redirection rules) redirects intercepted connections to this listener.
/// Each accepted connection is returned via [`TcpStack::accept`] for
/// processing by the proxy pipeline.
///
/// `feed()` is a no-op — the OS handles TCP state and packet routing.
pub struct SystemTcpStack {
    listener: TcpListener,
}

impl SystemTcpStack {
    /// Bind a TCP listener on the given address.
    ///
    /// Traffic redirection rules (iptables, pf, WFP) should redirect
    /// intercepted connections to this address.
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    /// Returns the local address the listener is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

impl TcpStack for SystemTcpStack {
    type Connection = TcpStream;

    /// No-op: the OS handles packet routing and TCP termination.
    async fn feed(&self, _packet: &[u8]) {}

    async fn accept(&self) -> Option<(Self::Connection, SocketAddress, SocketAddress)> {
        match self.listener.accept().await {
            Ok((stream, remote)) => {
                let local = self.listener.local_addr().ok()?;
                Some((stream, std_to_sockaddr(remote), std_to_sockaddr(local)))
            }
            Err(e) => {
                warn!("system tcp accept error: {e}");
                None
            }
        }
    }
}

// ── SystemUdpStack ─────────────────────────────────────────────────────

/// UDP stack backed by an OS-level UDP socket.
///
/// Binds a local UDP socket.  Inbound datagrams arrive via the OS socket
/// and are made available via [`UdpStack::recv_from`]; outbound datagrams
/// are sent via [`UdpStack::send_to`] through the same socket.
///
/// `feed()` is a no-op — UDP packets are received through the socket.
pub struct SystemUdpStack {
    socket: UdpSocket,
}

impl SystemUdpStack {
    /// Bind a UDP socket on the given address.
    pub async fn bind(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self { socket })
    }

    /// Returns the local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl UdpStack for SystemUdpStack {
    /// No-op: UDP packets arrive through the OS socket.
    async fn feed(&self, _packet: &[u8]) {}

    async fn recv_from(
        &self,
        buf: &mut [u8],
    ) -> Option<(usize, SocketAddress, SocketAddress)> {
        match self.socket.recv_from(buf).await {
            Ok((n, remote)) => {
                let local = self.socket.local_addr().ok()?;
                Some((n, std_to_sockaddr(remote), std_to_sockaddr(local)))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => {
                warn!("system udp recv error: {e}");
                None
            }
        }
    }

    async fn send_to(&self, data: &[u8], _src: SocketAddress, dst: SocketAddress) {
        let target = SocketAddr::new(
            match dst.ip {
                zero_traits::IpAddress::V4(o) => std::net::IpAddr::V4(o.into()),
                zero_traits::IpAddress::V6(o) => std::net::IpAddr::V6(o.into()),
            },
            dst.port,
        );
        if let Err(e) = self.socket.send_to(data, target).await {
            warn!("system udp send error: {e}");
        }
    }
}
