//! Mieru inbound — encrypted handshake + AEAD-framed relay.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use mieru::{
    build_data_segment, DataMetadata, MieruCipher, MieruInbound, MieruSession,
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

// ── Client stream wrapper ────────────────────────────────────────────

/// Wraps a `TcpRelayStream` carrying the Mieru session cipher state
/// for the server→client (download) direction.
pub(crate) struct MieruClientStream {
    inner: TcpRelayStream,
    /// Cipher for server→client (encrypt download).
    server_cipher: MieruCipher,
    /// Cipher for client→server (decrypt upload).
    client_cipher: MieruCipher,
    /// Mieru session tracking.
    mieru_session: MieruSession,
    /// Whether the first server→client nonce has been sent.
    s2c_nonce_sent: bool,
    /// Buffered decrypted data from a partial segment read.
    read_buf: Vec<u8>,
    read_pos: usize,
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

        // Try to read and decrypt a Mieru data segment from the client
        let mut raw = vec![0u8; 8192];
        let mut read_buf = ReadBuf::new(&mut raw);
        match Pin::new(&mut this.inner).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled().len();
                if filled == 0 {
                    return Poll::Ready(Ok(())); // EOF
                }
                raw.truncate(filled);

                // Decrypt client→server data segment
                match decrypt_client_data(
                    &raw,
                    &mut this.client_cipher,
                    !this.read_pos > 0, // first read has nonce
                ) {
                    Ok(payload) => {
                        let n = payload.len().min(buf.remaining());
                        buf.put_slice(&payload[..n]);
                        if n < payload.len() {
                            this.read_buf = payload[n..].to_vec();
                            this.read_pos = 0;
                        }
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for MieruClientStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Encrypt and frame as Mieru server→client data segment
        let this = Pin::into_inner(self);
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
                Pin::new(&mut this.inner).poll_write(cx, &segment)
            }
            Err(_) => Poll::Ready(Err(io::Error::other("mieru encrypt failed"))),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ── Handler ──────────────────────────────────────────────────────────

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

        let mut session = accept.session;
        let mut sa = zero_core::SessionAuth::new("mieru");
        sa.principal_key = Some("mieru".to_owned());
        session.apply_auth(sa);

        Ok((
            session,
            MieruClientStream {
                inner: metered.into_inner(),
                server_cipher: accept.server_cipher,
                client_cipher: accept.client_cipher,
                mieru_session: accept.mieru_session,
                s2c_nonce_sent: true, // Response nonce already sent in accept
                read_buf: accept.remaining_payload,
                read_pos: 0,
            },
        ))
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

// ── Listener ─────────────────────────────────────────────────────────

impl Proxy {
    pub(crate) async fn run_mieru_listener(
        &self,
        inbound: InboundConfig,
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

        let listener = bind_listener(&inbound).await?;
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

// ── UDP relay ────────────────────────────────────────────────────────

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

// ── AEAD relay helpers ────────────────────────────────────────────────

fn decrypt_client_data(
    data: &[u8],
    cipher: &mut MieruCipher,
    include_nonce: bool,
) -> io::Result<Vec<u8>> {
    cipher
        .decrypt(include_nonce, data)
        .map_err(|e| io::Error::other(format!("mieru decrypt: {e}")))
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
