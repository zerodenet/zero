//! User-space TCP termination stack.
//!
//! Implements [`TcpStack`] by maintaining a minimal TCP state machine
//! per connection.  Raw IP packets arrive via [`feed`]; the stack
//! completes three-way handshakes, extracts payload, and makes
//! established connections available via [`accept`].
//!
//! [`feed`]: TcpStack::feed
//! [`accept`]: TcpStack::accept

use std::collections::HashMap;
use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, Mutex};
use tracing::warn;

use zero_traits::{SocketAddress, TcpStack};

use crate::packet::{self, tcp_flags, Endpoint, ParsedTcp};

// ── ISS generator ─────────────────────────────────────────────────────

static NEXT_ISS: AtomicU32 = AtomicU32::new(1_000_000);

fn next_iss() -> u32 {
    NEXT_ISS.fetch_add(128_000, Ordering::Relaxed)
}

// ── Connection key ────────────────────────────────────────────────────

/// (src_ip, src_port, dst_ip, dst_port) — as seen in the incoming packet.
type ConnKey = (IpAddr, u16, IpAddr, u16);

fn key_from_parsed(t: &ParsedTcp) -> ConnKey {
    (t.src.ip, t.src.port, t.dst.ip, t.dst.port)
}

fn key_reversed(k: &ConnKey) -> ConnKey {
    (k.2, k.3, k.0, k.1)
}

fn endpoint_to_sockaddr(ep: &Endpoint) -> SocketAddress {
    let ip = match ep.ip {
        IpAddr::V4(v4) => zero_traits::IpAddress::V4(v4.octets()),
        IpAddr::V6(v6) => zero_traits::IpAddress::V6(v6.octets()),
    };
    SocketAddress::new(ip, ep.port)
}

// ── TCP state ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TcpState {
    /// We sent SYN-ACK, waiting for ACK from client.
    SynReceived,
    /// Three-way handshake complete, data transfer.
    Established,
    /// Received FIN from client, awaiting teardown.
    CloseWait,
}

/// Per-connection state tracked by the stack.
struct Conn {
    state: TcpState,
    /// Initial send sequence number (our side).
    iss: u32,
    /// Next sequence number to send (our side).
    snd_nxt: u32,
    /// Next expected receive sequence number (client side).
    rcv_nxt: u32,
    /// Sends inbound payload toward the proxy (UserTcpStream read side).
    data_tx: mpsc::Sender<Vec<u8>>,
    /// Read-side receiver — extracted when transitioning to Established
    /// and passed into the `UserTcpStream`.
    data_rx: Option<mpsc::Receiver<Vec<u8>>>,
}

// ── UserTcpStream ─────────────────────────────────────────────────────

/// A TCP stream bridging the user-space stack to the proxy pipeline.
///
/// - `AsyncRead` returns data received from the application (via TUN).
/// - `AsyncWrite` wraps data in TCP segments and sends them through
///   the outbound packet channel (back to TUN).
pub struct UserTcpStream {
    /// Data from application (proxy reads this).
    read_rx: Mutex<mpsc::Receiver<Vec<u8>>>,
    /// Connection metadata + outbound writer (proxy writes this).
    write: Mutex<TcpWrite>,
}

struct TcpWrite {
    /// Outbound packet channel (→ TUN writer task).
    outbound: mpsc::Sender<Vec<u8>>,
    /// Our IP (server side).
    src_ip: IpAddr,
    /// Application IP (client side).
    dst_ip: IpAddr,
    sport: u16,
    dport: u16,
    /// Next send sequence.
    snd_nxt: u32,
    /// Receive sequence (for ACK number).
    rcv_nxt: u32,
    /// Have we sent FIN?
    fin_sent: bool,
}

impl UserTcpStream {
    fn new(
        data_rx: mpsc::Receiver<Vec<u8>>,
        outbound: mpsc::Sender<Vec<u8>>,
        conn_key: &ConnKey,
        iss: u32,
        rcv_nxt: u32,
    ) -> Self {
        // conn_key = (app_ip, app_port, our_ip, our_port)
        // We send FROM our side TO app side:
        let rev = key_reversed(conn_key);
        Self {
            read_rx: Mutex::new(data_rx),
            write: Mutex::new(TcpWrite {
                outbound,
                src_ip: rev.0, // our_ip
                dst_ip: rev.2, // app_ip
                sport: rev.1,  // our_port
                dport: rev.3,  // app_port
                snd_nxt: iss.wrapping_add(1), // first data byte after SYN-ACK
                rcv_nxt,
                fin_sent: false,
            }),
        }
    }
}

impl AsyncRead for UserTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut rx = match self.read_rx.try_lock() {
            Ok(r) => r,
            Err(_) => return Poll::Pending,
        };
        match rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let n = data.len().min(buf.remaining());
                buf.put_slice(&data[..n]);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for UserTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut w = match self.write.try_lock() {
            Ok(w) => w,
            Err(_) => return Poll::Pending,
        };
        if w.fin_sent {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "connection closed",
            )));
        }
        let pkt = packet::build_tcp(
            w.src_ip, w.dst_ip,
            w.sport, w.dport,
            w.snd_nxt, w.rcv_nxt,
            tcp_flags::PSH | tcp_flags::ACK,
            data,
        );
        match w.outbound.try_send(pkt) {
            Ok(()) => {
                w.snd_nxt = w.snd_nxt.wrapping_add(data.len() as u32);
                Poll::Ready(Ok(data.len()))
            }
            Err(mpsc::error::TrySendError::Full(_)) => Poll::Pending,
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut w = match self.write.try_lock() {
            Ok(w) => w,
            Err(_) => return Poll::Pending,
        };
        if w.fin_sent {
            return Poll::Ready(Ok(()));
        }
        let pkt = packet::build_tcp(
            w.src_ip, w.dst_ip,
            w.sport, w.dport,
            w.snd_nxt, w.rcv_nxt,
            tcp_flags::FIN | tcp_flags::ACK,
            &[],
        );
        let _ = w.outbound.try_send(pkt);
        w.fin_sent = true;
        Poll::Ready(Ok(()))
    }
}

// ── Established connection ────────────────────────────────────────────

struct ReadyConn {
    stream: UserTcpStream,
    src: SocketAddress,
    dst: SocketAddress,
}

// ── UserTcpStack ──────────────────────────────────────────────────────

/// User-space TCP termination stack.
///
/// Implements [`TcpStack`].  Feed raw IP packets; accept established
/// connections.  Maintains a minimal per-connection TCP state machine
/// and emits response packets (SYN-ACK, ACK, FIN, RST) through an
/// internal outbound channel.
pub struct UserTcpStack {
    connections: Mutex<HashMap<ConnKey, Conn>>,
    accept_tx: mpsc::Sender<ReadyConn>,
    accept_rx: Mutex<mpsc::Receiver<ReadyConn>>,
    outbound: mpsc::Sender<Vec<u8>>,
    mss: u16,
}

impl UserTcpStack {
    pub(crate) fn new(outbound: mpsc::Sender<Vec<u8>>, mss: u16) -> Self {
        let (tx, rx) = mpsc::channel::<ReadyConn>(64);
        Self {
            connections: Mutex::new(HashMap::new()),
            accept_tx: tx,
            accept_rx: Mutex::new(rx),
            outbound,
            mss,
        }
    }

    /// Send a response packet.  Silently drops if the channel is full.
    fn send_response(&self, pkt: Vec<u8>) {
        if let Err(e) = self.outbound.try_send(pkt) {
            warn!("tcp stack outbound full: {e}");
        }
    }
}

impl TcpStack for UserTcpStack {
    type Connection = UserTcpStream;

    async fn feed(&self, packet: &[u8]) {
        if packet::ip_protocol(packet) != Some(packet::IPPROTO_TCP) {
            return;
        }
        let tcp = match packet::parse_tcp(packet) {
            Some(t) => t,
            None => return,
        };
        let key = key_from_parsed(&tcp);
        let rev = key_reversed(&key);

        let mut conns = self.connections.lock().await;

        // ── RST: tear down immediately ──
        if tcp.rst {
            conns.remove(&key);
            return;
        }

        // ── Existing connection ──
        if let Some(conn) = conns.get_mut(&key) {
            match conn.state {
                TcpState::SynReceived => {
                    // Expect ACK completing the handshake.
                    if !tcp.ack_flag || tcp.syn {
                        return;
                    }
                    conn.state = TcpState::Established;

                    // Extract the receiver that's been waiting since SYN.
                    let data_rx = conn.data_rx.take().expect("data_rx present in SynReceived");

                    let stream = UserTcpStream::new(
                        data_rx,
                        self.outbound.clone(),
                        &key,
                        conn.iss,
                        conn.rcv_nxt,
                    );

                    let src = endpoint_to_sockaddr(&tcp.src);
                    let dst = endpoint_to_sockaddr(&tcp.dst);
                    if self.accept_tx.try_send(ReadyConn { stream, src, dst }).is_err() {
                        warn!("tcp accept channel full, dropping connection");
                        conns.remove(&key);
                    }
                }
                TcpState::Established => {
                    // Data transfer.
                    if !tcp.payload.is_empty() {
                        // Forward payload to proxy.
                        if conn.data_tx.try_send(tcp.payload.to_vec()).is_err() {
                            // Channel full — proxy is slow.  Drop the packet;
                            // the client will retransmit.
                            warn!("tcp conn data channel full, dropping segment");
                            return;
                        }
                        conn.rcv_nxt = conn.rcv_nxt.wrapping_add(tcp.payload.len() as u32);
                    }
                    // ACK.
                    let ack = packet::build_tcp(
                        rev.0, rev.2, rev.1, rev.3,
                        conn.snd_nxt, conn.rcv_nxt,
                        tcp_flags::ACK, &[],
                    );
                    self.send_response(ack);

                    // FIN.
                    if tcp.fin {
                        conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                        let fin = packet::build_tcp(
                            rev.0, rev.2, rev.1, rev.3,
                            conn.snd_nxt, conn.rcv_nxt,
                            tcp_flags::FIN | tcp_flags::ACK, &[],
                        );
                        self.send_response(fin);
                        conns.remove(&key);
                    }
                }
                TcpState::CloseWait => {
                    // Already closing; ignore.
                }
            }
            return;
        }

        // ── New connection: must be SYN ──
        if !tcp.syn {
            return;
        }

        let iss = next_iss();
        let rcv_nxt = tcp.seq.wrapping_add(1);

        // SYN-ACK.
        let syn_ack = packet::build_tcp(
            rev.0, rev.2, rev.1, rev.3,
            iss, rcv_nxt,
            tcp_flags::SYN | tcp_flags::ACK, &[],
        );

        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(256);

        conns.insert(key, Conn {
            state: TcpState::SynReceived,
            iss,
            snd_nxt: iss.wrapping_add(1),
            rcv_nxt,
            data_tx,
            data_rx: Some(data_rx),
        });

        self.send_response(syn_ack);
    }

    async fn accept(&self) -> Option<(Self::Connection, SocketAddress, SocketAddress)> {
        let mut rx = self.accept_rx.lock().await;
        let conn = rx.recv().await?;
        Some((conn.stream, conn.src, conn.dst))
    }
}
