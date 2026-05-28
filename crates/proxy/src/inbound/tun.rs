//! TUN inbound — virtual network interface with IPv4/IPv6 dual-stack.
//!
//! Reads raw IP packets from a platform TUN device, dispatches TCP
//! connections through `serve_inbound()`, and forwards UDP datagrams
//! through a local relay socket.  Maintains a minimal TCP state machine.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinSet;
use tokio::time::interval;
use tracing::{error, info, warn};

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_tun::TunDevice;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::{Proxy, TunInfo};

// ── Packet parsing ─────────────────────────────────────────────────────

enum IpInfo {
    V4 { src: Ipv4Addr, dst: Ipv4Addr, header_len: u16 },
    V6 { src: Ipv6Addr, dst: Ipv6Addr },
}

struct TcpInfo {
    src_port: u16, dst_port: u16,
    syn: bool, rst: bool, fin: bool,
    seq: u32, data_off: u16,
}

struct UdpInfo {
    src_port: u16, dst_port: u16,
    payload_start: usize, payload_len: usize,
}

fn parse_tcp_header(buf: &[u8], ip_header_len: u16) -> Option<TcpInfo> {
    let off = ip_header_len as usize;
    if buf.len() < off + 20 { return None; }
    let h = &buf[off..];
    let flags = h[13];
    Some(TcpInfo {
        src_port: u16::from_be_bytes([h[0], h[1]]),
        dst_port: u16::from_be_bytes([h[2], h[3]]),
        syn: (flags & 0x02) != 0, rst: (flags & 0x04) != 0, fin: (flags & 0x01) != 0,
        seq: u32::from_be_bytes([h[4], h[5], h[6], h[7]]),
        data_off: ((h[12] >> 4) & 0x0f) as u16 * 4,
    })
}

fn parse_ip_tcp(buf: &[u8]) -> Option<(IpInfo, TcpInfo)> {
    if buf.len() < 40 { return None; }
    match buf[0] >> 4 {
        4 => {
            let ihl = (buf[0] & 0x0f) as u16 * 4;
            if buf.len() < ihl as usize + 20 || buf[9] != 6 { return None; }
            let ip = IpInfo::V4 {
                src: Ipv4Addr::new(buf[12], buf[13], buf[14], buf[15]),
                dst: Ipv4Addr::new(buf[16], buf[17], buf[18], buf[19]),
                header_len: ihl,
            };
            Some((ip, parse_tcp_header(buf, ihl)?))
        }
        6 => {
            if buf.len() < 60 || buf[6] != 6 { return None; }
            let mut s = [0u8; 16]; s.copy_from_slice(&buf[8..24]);
            let mut d = [0u8; 16]; d.copy_from_slice(&buf[24..40]);
            Some((IpInfo::V6 { src: Ipv6Addr::from(s), dst: Ipv6Addr::from(d) }, parse_tcp_header(buf, 40)?))
        }
        _ => None,
    }
}

fn parse_ip_udp(buf: &[u8]) -> Option<(IpInfo, UdpInfo)> {
    if buf.len() < 28 { return None; }
    match buf[0] >> 4 {
        4 => {
            let ihl = (buf[0] & 0x0f) as u16 * 4;
            if buf.len() < ihl as usize + 8 || buf[9] != 17 { return None; }
            let ip = IpInfo::V4 {
                src: Ipv4Addr::new(buf[12], buf[13], buf[14], buf[15]),
                dst: Ipv4Addr::new(buf[16], buf[17], buf[18], buf[19]),
                header_len: ihl,
            };
            let off = ihl as usize;
            Some((ip, UdpInfo {
                src_port: u16::from_be_bytes([buf[off], buf[off+1]]),
                dst_port: u16::from_be_bytes([buf[off+2], buf[off+3]]),
                payload_start: off + 8, payload_len: buf.len().saturating_sub(off + 8),
            }))
        }
        6 => {
            if buf.len() < 48 { return None; }
            let mut nh = buf[6]; let mut off = 40usize;
            while nh != 17 && off + 8 <= buf.len() {
                nh = buf[off]; off += (buf[off + 1] as usize) * 8 + 8;
            }
            if nh != 17 || off + 8 > buf.len() { return None; }
            let mut s = [0u8; 16]; s.copy_from_slice(&buf[8..24]);
            let mut d = [0u8; 16]; d.copy_from_slice(&buf[24..40]);
            Some((IpInfo::V6 { src: Ipv6Addr::from(s), dst: Ipv6Addr::from(d) }, UdpInfo {
                src_port: u16::from_be_bytes([buf[off], buf[off+1]]),
                dst_port: u16::from_be_bytes([buf[off+2], buf[off+3]]),
                payload_start: off + 8, payload_len: buf.len().saturating_sub(off + 8),
            }))
        }
        _ => None,
    }
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
    async fn accept(&self, _: crate::transport::TcpRelayStream) -> Result<(Session, Self::ClientStream), EngineError> {
        Err(EngineError::Io(io::Error::new(io::ErrorKind::Unsupported, "tun accept handled inline")))
    }
    async fn send_ok(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
    async fn send_blocked(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
    async fn send_upstream_failure(&self, _: &mut Self::ClientStream) -> Result<(), EngineError> { Ok(()) }
}

// ── Packet builders ────────────────────────────────────────────────────

fn build_tcp_pkt(src: IpAddr, dst: IpAddr, sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    match (src, dst) {
        (IpAddr::V4(s), IpAddr::V4(d)) => build_tcp_v4(s, d, sport, dport, seq, ack, flags, payload),
        (IpAddr::V6(s), IpAddr::V6(d)) => build_tcp_v6(s, d, sport, dport, seq, ack, flags, payload),
        _ => vec![],
    }
}

fn build_tcp_v4(src: Ipv4Addr, dst: Ipv4Addr, sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    let pl = payload.len(); let mut p = vec![0u8; 40 + pl];
    p[0] = 0x45; p[2] = ((40 + pl) >> 8) as u8; p[3] = (40 + pl) as u8; p[8] = 64; p[9] = 6;
    p[12..16].copy_from_slice(&src.octets()); p[16..20].copy_from_slice(&dst.octets());
    p[20..22].copy_from_slice(&sport.to_be_bytes()); p[22..24].copy_from_slice(&dport.to_be_bytes());
    p[24..28].copy_from_slice(&seq.to_be_bytes()); p[28..32].copy_from_slice(&ack.to_be_bytes());
    p[32] = 0x50; p[33] = flags;
    if pl > 0 { p[40..].copy_from_slice(payload); }
    p
}

fn build_tcp_v6(src: Ipv6Addr, dst: Ipv6Addr, sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    let pl = payload.len(); let tcp_total = 20 + pl; let mut p = vec![0u8; 40 + tcp_total];
    p[0] = 0x60; p[4..6].copy_from_slice(&(tcp_total as u16).to_be_bytes()); p[6] = 6; p[7] = 64;
    p[8..24].copy_from_slice(&src.octets()); p[24..40].copy_from_slice(&dst.octets());
    let o = 40;
    p[o..o+2].copy_from_slice(&sport.to_be_bytes()); p[o+2..o+4].copy_from_slice(&dport.to_be_bytes());
    p[o+4..o+8].copy_from_slice(&seq.to_be_bytes()); p[o+8..o+12].copy_from_slice(&ack.to_be_bytes());
    p[o+12] = 0x50; p[o+13] = flags;
    if pl > 0 { p[o+20..].copy_from_slice(payload); }
    p
}

fn build_udp_pkt(src: IpAddr, dst: IpAddr, sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    match (src, dst) {
        (IpAddr::V4(s), IpAddr::V4(d)) => build_udp_v4(s, d, sport, dport, payload),
        (IpAddr::V6(s), IpAddr::V6(d)) => build_udp_v6(s, d, sport, dport, payload),
        _ => vec![],
    }
}

fn build_udp_v4(src: Ipv4Addr, dst: Ipv4Addr, sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let pl = payload.len(); let udp_total = 8 + pl; let mut p = vec![0u8; 20 + udp_total];
    p[0] = 0x45; p[2] = ((20 + udp_total) >> 8) as u8; p[3] = (20 + udp_total) as u8;
    p[8] = 64; p[9] = 17;
    p[12..16].copy_from_slice(&src.octets()); p[16..20].copy_from_slice(&dst.octets());
    p[20..22].copy_from_slice(&sport.to_be_bytes()); p[22..24].copy_from_slice(&dport.to_be_bytes());
    p[24..26].copy_from_slice(&(udp_total as u16).to_be_bytes());
    if pl > 0 { p[28..].copy_from_slice(payload); }
    p
}

fn build_udp_v6(src: Ipv6Addr, dst: Ipv6Addr, sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let pl = payload.len(); let udp_total = 8 + pl; let mut p = vec![0u8; 40 + udp_total];
    p[0] = 0x60; p[4..6].copy_from_slice(&(udp_total as u16).to_be_bytes()); p[6] = 17; p[7] = 64;
    p[8..24].copy_from_slice(&src.octets()); p[24..40].copy_from_slice(&dst.octets());
    let o = 40;
    p[o..o+2].copy_from_slice(&sport.to_be_bytes()); p[o+2..o+4].copy_from_slice(&dport.to_be_bytes());
    p[o+4..o+6].copy_from_slice(&(udp_total as u16).to_be_bytes());
    if pl > 0 { p[o+8..].copy_from_slice(payload); }
    p
}

// ── Connection state ──────────────────────────────────────────────────

type ConnKey = (SocketAddr, SocketAddr);

struct ConnState {
    tx: mpsc::Sender<Vec<u8>>,
    last_active: std::time::Instant,
}

const CONN_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

// ── TCP handler ───────────────────────────────────────────────────────

async fn handle_tcp(
    device: &Arc<Mutex<impl TunDevice + 'static>>,
    connections: &Arc<Mutex<HashMap<ConnKey, ConnState>>>,
    proxy: &Proxy, tag: &str, tasks: &mut JoinSet<()>,
    ip: IpInfo, tcp: TcpInfo, buf: &[u8],
) {
    let n = buf.len();
    let (src_ip, dst_ip, header_len) = match &ip {
        IpInfo::V4 { src, dst, header_len } => (IpAddr::V4(*src), IpAddr::V4(*dst), *header_len),
        IpInfo::V6 { src, dst } => (IpAddr::V6(*src), IpAddr::V6(*dst), 40),
    };
    let endpoint = SocketAddr::new(dst_ip, tcp.dst_port);
    let src = SocketAddr::new(src_ip, tcp.src_port);
    let key: ConnKey = (src, endpoint);
    let pl_start = header_len as usize + tcp.data_off as usize;
    let payload = if n > pl_start { &buf[pl_start..n] } else { &[] };

    let mut conns = connections.lock().await;
    if let Some(conn) = conns.get_mut(&key) {
        conn.last_active = std::time::Instant::now();
        if !payload.is_empty() { let _ = conn.tx.send(payload.to_vec()).await; }
        if tcp.rst || tcp.fin { conns.remove(&key); }
        return;
    }
    if !tcp.syn { return; }

    let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(64);
    let (resp_tx, mut resp_rx) = mpsc::channel::<Vec<u8>>(64);
    conns.insert(key, ConnState { tx: data_tx, last_active: std::time::Instant::now() });
    drop(conns);

    let tun = device.clone();
    let rs = dst_ip; let rd = src_ip; let rsp = tcp.dst_port; let rdp = tcp.src_port;
    let server_seq: u32 = 42_000_000;
    let client_ack = tcp.seq.wrapping_add(1);
    tasks.spawn(async move {
        let mut seq = server_seq.wrapping_add(1);
        let _ = tun.lock().await.write(&build_tcp_pkt(rs, rd, rsp, rdp, server_seq, client_ack, 0x12, &[])).await;
        while let Some(data) = resp_rx.recv().await {
            let _ = tun.lock().await.write(&build_tcp_pkt(rs, rd, rsp, rdp, seq, client_ack, 0x18, &data)).await;
            seq = seq.wrapping_add(data.len() as u32);
        }
        let _ = tun.lock().await.write(&build_tcp_pkt(rs, rd, rsp, rdp, seq, client_ack, 0x11, &[])).await;
    });

    let session = Session::new(0, match dst_ip {
        IpAddr::V4(v4) => Address::Ipv4(v4.octets()),
        IpAddr::V6(v6) => Address::Ipv6(v6.octets()),
    }, tcp.dst_port, Network::Tcp, ProtocolType::Unknown);
    let stream = TunTcpStream { rx: Mutex::new(data_rx), tx: resp_tx };
    let p = proxy.clone(); let t = tag.to_owned();
    tasks.spawn(async move { let _ = serve_inbound(&p, session, stream, &TunProtocol, &t, Some(src)).await; });
}

// ── UDP handler ───────────────────────────────────────────────────────

async fn handle_udp(
    device: &Arc<Mutex<impl TunDevice + 'static>>,
    proxy: &Proxy, tag: &str,
    ip: IpInfo, udp: UdpInfo, buf: &[u8],
    relay_sock: &UdpSocket,
) {
    let (src_ip, dst_ip, _header_len) = match &ip {
        IpInfo::V4 { src, dst, header_len } => (IpAddr::V4(*src), IpAddr::V4(*dst), *header_len),
        IpInfo::V6 { src, dst } => (IpAddr::V6(*src), IpAddr::V6(*dst), 40),
    };
    if udp.payload_len == 0 { return; }

    let payload = &buf[udp.payload_start..udp.payload_start + udp.payload_len];
    let target = SocketAddr::new(dst_ip, udp.dst_port);

    // Forward to target via relay socket.
    if let Err(e) = relay_sock.send_to(payload, target).await {
        warn!(error = %e, "tun udp send_to failed");
        return;
    }

    // Read response (non-blocking, one shot).
    let mut resp_buf = [0u8; 65536];
    match relay_sock.try_recv_from(&mut resp_buf) {
        Ok((n, from)) if from == target => {
            let pkt = build_udp_pkt(dst_ip, src_ip, udp.dst_port, udp.src_port, &resp_buf[..n]);
            let _ = device.lock().await.write(&pkt).await;
        }
        _ => {} // no response or wrong sender — UDP is fire-and-forget
    }
}

// ── Dispatch loop ─────────────────────────────────────────────────────

async fn tun_loop(
    proxy: Proxy, device: impl TunDevice + 'static, tag: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let device = Arc::new(Mutex::new(device));
    let connections = Arc::new(Mutex::new(HashMap::<ConnKey, ConnState>::new()));
    let mut buf = vec![0u8; 65536];
    let mut tasks = JoinSet::new();
    let mut cleanup_tick = interval(Duration::from_secs(60));
    let relay_sock = UdpSocket::bind("0.0.0.0:0").await.expect("tun udp relay socket");

    loop {
        let n;
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() { info!("tun shutdown requested"); break; }
                continue;
            }
            _ = cleanup_tick.tick() => {
                let mut conns = connections.lock().await;
                conns.retain(|_, s| s.last_active.elapsed() < CONN_IDLE_TIMEOUT);
                continue;
            }
            r = async { device.lock().await.read(&mut buf).await } => {
                match r {
                    Ok(0) => break,
                    Ok(nb) => n = nb,
                    Err(e) => { error!(error = %e, "tun read"); break; }
                }
            }
        }

        if let Some((ip, tcp)) = parse_ip_tcp(&buf[..n]) {
            handle_tcp(&device, &connections, &proxy, &tag, &mut tasks, ip, tcp, &buf[..n]).await;
        } else if let Some((ip, udp)) = parse_ip_udp(&buf[..n]) {
            handle_udp(&device, &proxy, &tag, ip, udp, &buf[..n], &relay_sock).await;
        }
    }
    tasks.abort_all();
}

// ── Proxy entry ───────────────────────────────────────────────────────

impl Proxy {
    pub async fn start_tun(
        &self, name: Option<&str>, addr: &str, _mask: &str, _mtu: u16, tag: &str,
    ) -> Result<(), EngineError> {
        {
            let info = self.tun_info.lock().unwrap();
            if info.is_some() {
                return Err(EngineError::Io(io::Error::new(io::ErrorKind::AlreadyExists, "TUN is already running")));
            }
        }
        let device = zero_tun::create(name).map_err(EngineError::Io)?;
        let dn = device.name().to_owned();
        info!(inbound_tag = tag, name = %dn, addr = %addr, "tun device created");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        *self.tun_shutdown.lock().unwrap() = Some(shutdown_tx);
        *self.tun_info.lock().unwrap() = Some(TunInfo { name: dn, addr: addr.to_owned(), tag: tag.to_owned() });

        let proxy = self.clone(); let t = tag.to_owned();
        tokio::spawn(async move { tun_loop(proxy, device, t, shutdown_rx).await; });
        Ok(())
    }

    pub fn stop_tun(&self) -> Result<(), EngineError> {
        let mut s = self.tun_shutdown.lock().unwrap();
        if let Some(tx) = s.take() { let _ = tx.send(true); *self.tun_info.lock().unwrap() = None; Ok(()) }
        else { Err(EngineError::Io(io::Error::new(io::ErrorKind::NotFound, "TUN is not running"))) }
    }

    #[allow(dead_code)]
    pub(crate) fn tun_status(&self) -> Option<TunInfo> { self.tun_info.lock().unwrap().clone() }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_syn_v4() {
        let p = build_tcp_v4(Ipv4Addr::new(10,0,0,2), Ipv4Addr::new(1,2,3,4), 12345, 443, 100, 0, 0x02, b"");
        let (ip, tcp) = parse_ip_tcp(&p).expect("SYN v4");
        assert!(matches!(ip, IpInfo::V4{..})); assert!(tcp.syn); assert_eq!(tcp.seq, 100);
    }

    #[test]
    fn test_parse_udp_v4() {
        let p = build_udp_v4(Ipv4Addr::new(10,0,0,2), Ipv4Addr::new(8,8,8,8), 12345, 53, b"dns query");
        let (ip, udp) = parse_ip_udp(&p).expect("UDP v4");
        assert!(matches!(ip, IpInfo::V4{..})); assert_eq!(udp.dst_port, 53);
    }

    #[test]
    fn test_parse_syn_v6() {
        let s = Ipv6Addr::new(0xfd00,0,0,0,0,0,0,1);
        let d = Ipv6Addr::new(0x2606,0x4700,0,0,0,0,0x6810,0x1);
        let p = build_tcp_v6(s, d, 54321, 443, 500, 0, 0x02, b"");
        let (ip, tcp) = parse_ip_tcp(&p).expect("SYN v6");
        assert!(matches!(ip, IpInfo::V6{..})); assert!(tcp.syn);
    }

    #[test]
    fn test_parse_udp_v6() {
        let s = Ipv6Addr::LOCALHOST; let d = Ipv6Addr::LOCALHOST;
        let p = build_udp_v6(s, d, 1, 53, b"dns");
        let (ip, udp) = parse_ip_udp(&p).expect("UDP v6");
        assert_eq!(udp.dst_port, 53);
    }

    #[test]
    fn test_tcp_ignores_udp() {
        let p = build_udp_v4(Ipv4Addr::new(10,0,0,1), Ipv4Addr::new(8,8,8,8), 1, 53, b"x");
        assert!(parse_ip_tcp(&p).is_none());
    }

    #[test]
    fn test_udp_ignores_tcp() {
        let p = build_tcp_v4(Ipv4Addr::new(10,0,0,1), Ipv4Addr::new(8,8,8,8), 1, 80, 0, 0, 0x02, b"");
        assert!(parse_ip_udp(&p).is_none());
    }
}
