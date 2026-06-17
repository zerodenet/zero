//! Mieru inbound encrypted handshake and AEAD-framed relay.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use mieru::{
    build_data_segment, parse_segment, DataMetadata, MieruCipher, MieruInbound, MieruSession,
    DATA_SERVER_TO_CLIENT,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::DnsResolver;

use crate::logging::log_listener_connection_error;
use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

// Client stream wrapper.

/// Wraps a `TcpRelayStream` carrying the Mieru session cipher state
/// for the server-to-client (download) direction.
pub(crate) struct MieruClientStream {
    inner: TcpRelayStream,
    /// Cipher for server-to-client traffic (encrypt download).
    server_cipher: MieruCipher,
    /// Cipher for client-to-server traffic (decrypt upload).
    client_cipher: MieruCipher,
    /// Mieru session tracking.
    mieru_session: MieruSession,
    /// Whether the first server-to-client nonce has been sent.
    s2c_nonce_sent: bool,
    c2s_nonce_recv: bool,
    /// Buffered decrypted data from a partial segment read.
    read_buf: Vec<u8>,
    read_pos: usize,
    raw_read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    write_pos: usize,
    write_plain_len: usize,
}

impl AsyncRead for MieruClientStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);

        // Serve buffered decrypted data first
        if this.read_pos < this.read_buf.len() {
            let remaining = &this.read_buf[this.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            this.read_pos += n;
            if this.read_pos >= this.read_buf.len() {
                this.read_buf.clear();
                this.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        loop {
            let include_nonce = !this.c2s_nonce_recv;
            let mut cipher = this.client_cipher.clone();
            match parse_segment(&this.raw_read_buf, &mut cipher, include_nonce, false) {
                Ok((segment, consumed)) => {
                    this.client_cipher = cipher;
                    this.c2s_nonce_recv = true;
                    this.raw_read_buf.drain(..consumed);

                    let payload = segment.payload;
                    if payload.is_empty() {
                        continue;
                    }

                    let n = payload.len().min(buf.remaining());
                    buf.put_slice(&payload[..n]);
                    if n < payload.len() {
                        this.read_buf = payload[n..].to_vec();
                        this.read_pos = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(error) if error == zero_core::Error::Protocol("mieru: need more data") => {}
                Err(error) => {
                    return Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {error}"))));
                }
            }

            let before = this.raw_read_buf.len();
            this.raw_read_buf.resize(before + 8192, 0);
            let mut read_buf = ReadBuf::new(&mut this.raw_read_buf[before..]);
            match Pin::new(&mut this.inner).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let filled = read_buf.filled().len();
                    this.raw_read_buf.truncate(before + filled);
                    if filled == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(e)) => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Pending;
                }
            }
        }
    }
}

impl AsyncWrite for MieruClientStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = Pin::into_inner(self);

        if this.write_buf.is_empty() {
            let meta = DataMetadata {
                protocol_type: DATA_SERVER_TO_CLIENT,
                timestamp: MieruSession::timestamp_minutes(),
                session_id: this.mieru_session.session_id,
                sequence_number: this.mieru_session.next_send_seq(),
                unack_sequence: 0,
                window_size: 1024,
                fragment_number: 0,
                prefix_length: 0,
                payload_length: buf.len() as u16,
                suffix_length: 0,
            };
            match build_data_segment(&meta, buf, &mut this.server_cipher, !this.s2c_nonce_sent) {
                Ok(segment) => {
                    this.s2c_nonce_sent = true;
                    this.write_buf = segment;
                    this.write_pos = 0;
                    this.write_plain_len = buf.len();
                }
                Err(_) => return Poll::Ready(Err(io::Error::other("mieru encrypt failed"))),
            }
        }

        while this.write_pos < this.write_buf.len() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf[this.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mieru write zero",
                    )));
                }
                Poll::Ready(Ok(n)) => {
                    this.write_pos += n;
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        let written = this.write_plain_len;
        this.write_buf.clear();
        this.write_pos = 0;
        this.write_plain_len = 0;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Run the in-tunnel socks5 server side: read the client's socks5 request
/// directly (no greeting/auth — the mieru session is the auth), respond, and
/// return the requested target plus whether it is a UDP ASSOCIATE.
async fn socks5_serve<S>(stream: &mut S) -> io::Result<(zero_core::Address, u16, bool)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // Request: VER, CMD, RSV, ATYP, DST.ADDR, DST.PORT.
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(io::Error::other("mieru socks5: bad request version"));
    }
    let cmd = head[1];
    let target = match head[3] {
        0x01 => {
            let mut ip = [0u8; 4];
            stream.read_exact(&mut ip).await?;
            zero_core::Address::Ipv4(ip)
        }
        0x04 => {
            let mut ip = [0u8; 16];
            stream.read_exact(&mut ip).await?;
            zero_core::Address::Ipv6(ip)
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut d = vec![0u8; len[0] as usize];
            stream.read_exact(&mut d).await?;
            zero_core::Address::Domain(String::from_utf8_lossy(&d).into_owned())
        }
        _ => return Err(io::Error::other("mieru socks5: bad address type")),
    };
    let mut port_bytes = [0u8; 2];
    stream.read_exact(&mut port_bytes).await?;
    let port = u16::from_be_bytes(port_bytes);

    if cmd != 0x01 && cmd != 0x03 {
        return Err(io::Error::other(format!(
            "mieru socks5: unsupported command 0x{cmd:02x}"
        )));
    }

    // Reply: success, BND.ADDR = 0.0.0.0:0.
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    stream.flush().await?;

    Ok((target, port, cmd == 0x03))
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

        let mut client = MieruClientStream {
            inner: metered.into_inner(),
            server_cipher: accept.server_cipher,
            client_cipher: accept.client_cipher,
            mieru_session: accept.mieru_session,
            s2c_nonce_sent: true, // Response nonce already sent in accept
            c2s_nonce_recv: true,
            read_buf: accept.remaining_payload,
            read_pos: 0,
            raw_read_buf: Vec::new(),
            write_buf: Vec::new(),
            write_pos: 0,
            write_plain_len: 0,
        };

        // mieru conveys the proxy target via a socks5 request inside the tunnel.
        let (target, port, is_udp) = socks5_serve(&mut client)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru socks5: {e}"))))?;
        let network = if is_udp {
            zero_core::Network::Udp
        } else {
            zero_core::Network::Tcp
        };
        let mut session = Session::new(0, target, port, network, zero_core::ProtocolType::Mieru);
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

impl Proxy {
    pub(crate) async fn run_mieru_listener_with_bound(
        &self,
        inbound: InboundConfig,
        listener: zero_platform_tokio::TokioListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let users = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Mieru { users } => users
                .iter()
                .map(|u| (u.username.clone(), u.password.clone()))
                .collect::<Vec<_>>(),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "mieru listener requires mieru config",
                )))
            }
        };

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
                            let engine = self.clone();
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
        let mut session_map: std::collections::HashMap<
            std::net::SocketAddr,
            (zero_core::Address, u16),
        > = std::collections::HashMap::new();

        loop {
            tokio::select! {
                // Read decrypted data from Mieru client
                read = client.read(&mut read_buf) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = &read_buf[..n];
                            if let Ok(unwrapped) =
                                mieru::unwrap_udp_associate(data)
                            {
                                if let Ok(pkt) =
                                    socks5::parse_udp_packet(&unwrapped)
                                {
                                    let target_addr = match &pkt.target {
                                        zero_core::Address::Domain(domain) => {
                                            match self.resolver.resolve(domain).await {
                                                Ok(ips) => ips.first().copied().map(|ip| {
                                                    addr_from_ip(ip, pkt.port)
                                                }),
                                                Err(_) => None,
                                            }
                                        }
                                        zero_core::Address::Ipv4(ip) => Some(
                                            std::net::SocketAddr::new(
                                                std::net::IpAddr::V4(
                                                    std::net::Ipv4Addr::new(
                                                        ip[0], ip[1], ip[2], ip[3],
                                                    ),
                                                ),
                                                pkt.port,
                                            ),
                                        ),
                                        zero_core::Address::Ipv6(ip) => Some(
                                            std::net::SocketAddr::new(
                                                std::net::IpAddr::V6(
                                                    std::net::Ipv6Addr::from(*ip),
                                                ),
                                                pkt.port,
                                            ),
                                        ),
                                    };

                                    if let Some(addr) = target_addr {
                                        session_map.insert(
                                            addr,
                                            (pkt.target.clone(), pkt.port),
                                        );
                                        let _ = udp_socket.send_to(&pkt.payload, addr).await;
                                    }
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
                            if let Some((target, port)) = session_map.get(&sender) {
                                if let Ok(frame) = socks5::build_udp_packet(
                                    target, *port, &recv_buf[..n],
                                ) {
                                    let wrapped =
                                        mieru::wrap_udp_associate(&frame);
                                    if let Err(e) = client.write_all(&wrapped).await {
                                        tracing::warn!(
                                            error = %e, "mieru udp write error"
                                        );
                                        break;
                                    }
                                    if let Err(e) = client.flush().await {
                                        tracing::warn!(
                                            error = %e, "mieru udp flush error"
                                        );
                                        break;
                                    }
                                }
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

fn addr_from_ip(ip: zero_traits::IpAddress, port: u16) -> std::net::SocketAddr {
    match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), port)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), port)
        }
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
