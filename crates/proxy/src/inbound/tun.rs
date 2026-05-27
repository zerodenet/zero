//! TUN inbound — virtual network interface.
//!
//! Reads raw IP packets from a platform TUN device, parses TCP headers,
//! and dispatches each TCP connection through `serve_inbound()`.
//! Maintains a minimal TCP state machine for connection tracking.
//!
//! TODO: replace hand-rolled TCP with `smoltcp` for full TCP compliance
//!       (retransmission, window scaling, out-of-order handling).

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinSet;
use tracing::{error, info};

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_tun::TunDevice;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::{Proxy, TunInfo};

// ── Packet parsing ─────────────────────────────────────────────────────

struct Ipv4Info {
    src: Ipv4Addr,
    dst: Ipv4Addr,
    header_len: u16,
}

struct TcpInfo {
    src_port: u16,
    dst_port: u16,
    syn: bool,
    rst: bool,
    fin: bool,
    seq: u32,
    data_off: u16,
}

fn parse_ipv4_tcp(buf: &[u8]) -> Option<(Ipv4Info, TcpInfo)> {
    if buf.len() < 40 { return None; }
    let ver_ihl = buf[0];
    if (ver_ihl >> 4) != 4 { return None; }
    let ihl = (ver_ihl & 0x0f) as u16 * 4;
    if buf.len() < ihl as usize + 20 { return None; }
    if buf[9] != 6 { return None; } // TCP only
    let ip = Ipv4Info {
        src: Ipv4Addr::new(buf[12], buf[13], buf[14], buf[15]),
        dst: Ipv4Addr::new(buf[16], buf[17], buf[18], buf[19]),
        header_len: ihl,
    };
    let tcp_start = ihl as usize;
    let tcp = &buf[tcp_start..];
    let data_off = ((tcp[12] >> 4) & 0x0f) as u16 * 4;
    let flags = tcp[13];
    let tcp_info = TcpInfo {
        src_port: u16::from_be_bytes([tcp[0], tcp[1]]),
        dst_port: u16::from_be_bytes([tcp[2], tcp[3]]),
        syn: (flags & 0x02) != 0,
        rst: (flags & 0x04) != 0,
        fin: (flags & 0x01) != 0,
        seq: u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]),
        data_off,
    };
    Some((ip, tcp_info))
}

// ── Bridged stream ────────────────────────────────────────────────────

struct TunTcpStream {
    rx: Mutex<mpsc::Receiver<Vec<u8>>>,
    tx: mpsc::Sender<Vec<u8>>,
}

impl AsyncRead for TunTcpStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let mut rx = match self.rx.try_lock() { Ok(r) => r, Err(_) => return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, "lock"))), };
        match rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => { let n = data.len().min(buf.remaining()); buf.put_slice(&data[..n]); Poll::Ready(Ok(())) }
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for TunTcpStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Full(_)) => { cx.waker().wake_by_ref(); Poll::Pending }
            Err(mpsc::error::TrySendError::Closed(_)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> { Poll::Ready(Ok(())) }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> { Poll::Ready(Ok(())) }
}

// ── Protocol handler ──────────────────────────────────────────────────

struct TunProtocol;

#[async_trait]
impl InboundProtocol for TunProtocol {
    type ClientStream = TunTcpStream;
    async fn accept(&self, _: crate::transport::TcpRelayStream) -> Result<(Session, Self::ClientStream), EngineError> { unreachable!() }
    async fn send_ok(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
    async fn send_blocked(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
    async fn send_upstream_failure(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
}

// ── TCP packet builder ────────────────────────────────────────────────

fn build_tcp_pkt(
    src: Ipv4Addr, dst: Ipv4Addr, sport: u16, dport: u16,
    seq: u32, ack: u32, flags: u8, payload: &[u8],
) -> Vec<u8> {
    let plen = payload.len();
    let mut p = vec![0u8; 40 + plen];
    p[0] = 0x45; p[2] = ((40 + plen) >> 8) as u8; p[3] = (40 + plen) as u8;
    p[8] = 64; p[9] = 6;
    p[12..16].copy_from_slice(&src.octets()); p[16..20].copy_from_slice(&dst.octets());
    p[20..22].copy_from_slice(&sport.to_be_bytes()); p[22..24].copy_from_slice(&dport.to_be_bytes());
    p[24..28].copy_from_slice(&seq.to_be_bytes()); p[28..32].copy_from_slice(&ack.to_be_bytes());
    p[32] = 0x50; p[33] = flags;
    if plen > 0 { p[40..].copy_from_slice(payload); }
    p
}

// ── Connection state ──────────────────────────────────────────────────

type ConnKey = (SocketAddr, SocketAddr);

struct ConnState {
    tx: mpsc::Sender<Vec<u8>>,
}

// ── Dispatch loop ─────────────────────────────────────────────────────

async fn tun_loop(
    proxy: Proxy, device: impl TunDevice + 'static, tag: String,
    shutdown: watch::Receiver<bool>,
) {
    let device = Arc::new(Mutex::new(device));
    let connections = Arc::new(Mutex::new(HashMap::<ConnKey, ConnState>::new()));
    let mut buf = vec![0u8; 65536];
    let mut tasks = JoinSet::new();

    loop {
        // Check shutdown
        if *shutdown.borrow() {
            info!("tun shutdown requested");
            break;
        }
        let n = {
            let mut dev = device.lock().await;
            match dev.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => { error!(error = %e, "tun read"); break; }
            }
        };
        let (ip, tcp) = match parse_ipv4_tcp(&buf[..n]) { Some(v) => v, None => continue };

        let endpoint = SocketAddr::new(IpAddr::V4(ip.dst), tcp.dst_port);
        let src = SocketAddr::new(IpAddr::V4(ip.src), tcp.src_port);
        let key: ConnKey = (src, endpoint);
        let payload_start = ip.header_len as usize + tcp.data_off as usize;
        let payload = if n > payload_start { &buf[payload_start..n] } else { &[] };

        let mut conns = connections.lock().await;
        if let Some(conn) = conns.get_mut(&key) {
            if !payload.is_empty() { let _ = conn.tx.send(payload.to_vec()).await; }
            if tcp.rst || tcp.fin { conns.remove(&key); }
            continue;
        }
        if !tcp.syn { continue; }

        // New connection.
        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(64);
        let (resp_tx, mut resp_rx) = mpsc::channel::<Vec<u8>>(64);
        conns.insert(key, ConnState { tx: data_tx });
        drop(conns);

        // Reply writer.
        let tun = device.clone();
        let reply_src = ip.dst; let reply_dst = ip.src;
        let reply_sport = tcp.dst_port; let reply_dport = tcp.src_port;
        let server_seq: u32 = 42_000_000;
        let client_ack = tcp.seq.wrapping_add(1);
        tasks.spawn(async move {
            let mut seq = server_seq.wrapping_add(1);
            // SYN-ACK
            let _ = tun.lock().await.write(&build_tcp_pkt(reply_src, reply_dst, reply_sport, reply_dport, server_seq, client_ack, 0x12, &[])).await;
            while let Some(data) = resp_rx.recv().await {
                let _ = tun.lock().await.write(&build_tcp_pkt(reply_src, reply_dst, reply_sport, reply_dport, seq, client_ack, 0x18, &data)).await;
                seq = seq.wrapping_add(data.len() as u32);
            }
            let _ = tun.lock().await.write(&build_tcp_pkt(reply_src, reply_dst, reply_sport, reply_dport, seq, client_ack, 0x11, &[])).await;
        });

        // serve_inbound
        let stream = TunTcpStream { rx: Mutex::new(data_rx), tx: resp_tx };
        let session = Session::new(0, Address::Ipv4(ip.dst.octets()), tcp.dst_port, Network::Tcp, ProtocolType::Unknown);
        let p = proxy.clone(); let t = tag.clone();
        tasks.spawn(async move { let _ = serve_inbound(&p, session, stream, &TunProtocol, &t, Some(src)).await; });
    }
    tasks.abort_all();
}

// ── Proxy entry ───────────────────────────────────────────────────────

impl Proxy {
    pub async fn start_tun(
        &self, name: Option<&str>, addr: &str, _mask: &str, _mtu: u16, tag: &str,
    ) -> Result<(), EngineError> {
        // Reject if already running.
        {
            let info = self.tun_info.lock().unwrap();
            if info.is_some() {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists, "TUN is already running",
                )));
            }
        }

        let device = zero_tun::create(name).map_err(EngineError::Io)?;
        let dev_name = device.name().to_owned();
        info!(inbound_tag = tag, name = %dev_name, addr = %addr, "tun device created");

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        *self.tun_shutdown.lock().unwrap() = Some(shutdown_tx);
        *self.tun_info.lock().unwrap() = Some(TunInfo {
            name: dev_name, addr: addr.to_owned(), tag: tag.to_owned(),
        });

        let proxy = self.clone();
        let t = tag.to_owned();
        tokio::spawn(async move { tun_loop(proxy, device, t, shutdown_rx).await; });

        Ok(())
    }

    pub fn stop_tun(&self) -> Result<(), EngineError> {
        let mut shutdown = self.tun_shutdown.lock().unwrap();
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(true);
            *self.tun_info.lock().unwrap() = None;
            Ok(())
        } else {
            Err(EngineError::Io(io::Error::new(
                io::ErrorKind::NotFound, "TUN is not running",
            )))
        }
    }

    #[allow(dead_code)]
    pub(crate) fn tun_status(&self) -> Option<TunInfo> {
        self.tun_info.lock().unwrap().clone()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_syn_packet() {
        let pkt = build_tcp_pkt(
            Ipv4Addr::new(10, 0, 0, 2), Ipv4Addr::new(1, 2, 3, 4),
            12345, 443, 100, 0, 0x02, b"",
        );
        let (ip, tcp) = parse_ipv4_tcp(&pkt).expect("parse SYN");
        assert_eq!(ip.src, Ipv4Addr::new(10, 0, 0, 2));
        assert_eq!(ip.dst, Ipv4Addr::new(1, 2, 3, 4));
        assert_eq!(tcp.src_port, 12345);
        assert_eq!(tcp.dst_port, 443);
        assert!(tcp.syn);
        assert!(!tcp.rst);
        assert!(!tcp.fin);
        assert_eq!(tcp.seq, 100);
    }

    #[test]
    fn test_parse_data_packet() {
        let pkt = build_tcp_pkt(
            Ipv4Addr::new(10, 0, 0, 2), Ipv4Addr::new(1, 2, 3, 4),
            12345, 443, 101, 1, 0x18, b"hello",
        );
        let (_, tcp) = parse_ipv4_tcp(&pkt).expect("parse data");
        assert!(!tcp.syn);
        assert_eq!(tcp.seq, 101);
    }

    #[test]
    fn test_parse_fin_packet() {
        let pkt = build_tcp_pkt(
            Ipv4Addr::new(10, 0, 0, 2), Ipv4Addr::new(1, 2, 3, 4),
            12345, 443, 200, 50, 0x11, b"",
        );
        let (_, tcp) = parse_ipv4_tcp(&pkt).expect("parse FIN");
        assert!(tcp.fin);
    }

    #[test]
    fn test_parse_ignores_udp() {
        let pkt = build_tcp_pkt(
            Ipv4Addr::new(10, 0, 0, 2), Ipv4Addr::new(1, 2, 3, 4),
            12345, 443, 0, 0, 0x02, b"",
        );
        let mut pkt2 = pkt.clone();
        pkt2[9] = 17; // change protocol to UDP
        assert!(parse_ipv4_tcp(&pkt2).is_none());
    }

    #[test]
    fn test_build_tcp_packet_roundtrip() {
        let payload = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let pkt = build_tcp_pkt(
            Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(93, 184, 216, 34),
            54321, 80, 1000, 0, 0x18, payload,
        );
        let (ip, tcp) = parse_ipv4_tcp(&pkt).expect("roundtrip");
        assert_eq!(ip.src, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(tcp.src_port, 54321);
        assert_eq!(tcp.dst_port, 80);
        assert_eq!(tcp.seq, 1000);
        // Extract payload
        let pl_start = (ip.header_len + tcp.data_off) as usize;
        assert_eq!(&pkt[pl_start..], payload);
    }
}
