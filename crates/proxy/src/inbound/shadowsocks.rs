//! Shadowsocks inbound — AEAD accept, route, AEAD-framed relay.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use ring::rand::SecureRandom;
use shadowsocks::{
    aead_decrypt_udp, aead_encrypt_udp, build_target_data, parse_target_data, CipherKind,
    ShadowsocksInbound,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{Address, Session};
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, RateLimiter, TcpRelayStream};

// ── Client stream wrapper (carries AEAD key + first-chunk payload) ────

pub(crate) struct SsClientStream {
    inner: TcpRelayStream,
    upload_key: Vec<u8>,
    download_key: Vec<u8>,
    next_upload_nonce: u64,
    response_salt: Vec<u8>,
    remaining: Vec<u8>,
}

impl AsyncRead for SsClientStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for SsClientStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ── Handler ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct ShadowsocksInboundHandler {
    ss_inbound: ShadowsocksInbound,
    cipher: CipherKind,
    password: Vec<u8>,
}

#[async_trait]
impl InboundProtocol for ShadowsocksInboundHandler {
    type ClientStream = SsClientStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        let accept = self
            .ss_inbound
            .accept_request(&mut metered, self.cipher, &self.password)
            .await?;

        let mut session = accept.session;
        let mut sa = zero_core::SessionAuth::new("shadowsocks");
        sa.principal_key = Some(String::from_utf8_lossy(&self.password).to_string());
        session.apply_auth(sa);

        let mut response_salt = vec![0u8; self.cipher.salt_len()];
        ring::rand::SystemRandom::new()
            .fill(&mut response_salt)
            .map_err(|_| {
                EngineError::Io(io::Error::other("shadowsocks: response salt random failed"))
            })?;
        let download_key = download_key_for_client(self.cipher, &self.password, &response_salt)
            .map_err(|e| EngineError::Io(io::Error::other(format!("shadowsocks: {e}"))))?;

        Ok((
            session,
            SsClientStream {
                inner: metered.into_inner(),
                upload_key: accept.session_key,
                download_key,
                next_upload_nonce: accept.next_upload_nonce,
                response_salt,
                remaining: accept.remaining_payload,
            },
        ))
    }

    async fn send_ok(&self, _client: &mut SsClientStream) -> Result<(), EngineError> {
        Ok(()) // Shadowsocks has no success response
    }

    async fn send_blocked(&self, _client: &mut SsClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut SsClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    /// Custom AEAD-framed relay: decrypt upload, encrypt download.
    /// Rate limiting uses the kernel's `RateLimiter` (unified GCRA).
    async fn relay(
        &self,
        client: SsClientStream,
        upstream: TcpRelayStream,
        proxy: &Proxy,
        session_id: u64,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        let response_salt = client.response_salt.clone();
        let (client_read, mut client_write) = tokio::io::split(client.inner);
        let (up_read, mut up_write) = tokio::io::split(upstream);

        if !client.remaining.is_empty() {
            if up_write.write_all(&client.remaining).await.is_ok() {
                let bytes = client.remaining.len() as u64;
                proxy.record_session_inbound_rx(session_id, bytes);
                proxy.record_session_outbound_tx(session_id, bytes);
            }
        }

        client_write.write_all(&response_salt).await?;
        client_write.flush().await?;

        let key_up = client.upload_key.clone();
        let key_down = client.download_key;
        let cipher = self.cipher;
        let upload_nonce = client.next_upload_nonce;
        let upload_proxy = proxy.clone();
        let upload = tokio::spawn(async move {
            let _ = ss_decrypt_upload(
                client_read,
                up_write,
                cipher,
                key_up,
                upload_nonce,
                up_bps,
                move |bytes| {
                    upload_proxy.record_session_inbound_rx(session_id, bytes);
                    upload_proxy.record_session_outbound_tx(session_id, bytes);
                },
            )
            .await;
        });
        let download_proxy = proxy.clone();
        let download = tokio::spawn(async move {
            let _ = ss_encrypt_download(
                up_read,
                client_write,
                cipher,
                key_down,
                down_bps,
                move |bytes| {
                    download_proxy.record_session_outbound_rx(session_id, bytes);
                    download_proxy.record_session_inbound_tx(session_id, bytes);
                },
            )
            .await;
        });

        let _ = tokio::try_join!(upload, download);
        Ok(())
    }
}

// ── Listener ────────────────────────────────────────────────────────────

impl Proxy {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_shadowsocks_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (password, cipher_str, _up_bps, _down_bps) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Shadowsocks {
                password,
                cipher,
                up_bps,
                down_bps,
            } => (password.clone(), cipher.clone(), *up_bps, *down_bps),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "shadowsocks listener requires shadowsocks config",
                )))
            }
        };

        let cipher = CipherKind::from_str(&cipher_str).ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown shadowsocks cipher: {cipher_str}"),
            ))
        })?;

        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;

        let udp_socket = match UdpSocket::bind(&format!(
            "{}:{}",
            inbound.listen.address, inbound.listen.port
        ))
        .await
        {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                warn!(error = %e, "shadowsocks: failed to bind UDP socket, UDP disabled");
                None
            }
        };

        let handler = ShadowsocksInboundHandler {
            ss_inbound: ShadowsocksInbound,
            cipher,
            password: password.clone().into_bytes(),
        };

        let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "shadowsocks",
            cipher = %cipher_str,
            listen = %local_addr,
            udp = udp_socket.is_some(),
            "inbound listener ready"
        );

        if let Some(udp) = udp_socket.as_ref() {
            let engine = self.clone();
            let tag = inbound.tag.clone();
            let password = password.clone();
            let udp = udp.clone();
            connections
                .spawn(async move { engine.ss_udp_relay_loop(udp, &tag, &password, cipher).await });
        }

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
                                        let _ = serve_inbound(
                                            &engine, session, client, &handler,
                                            &tag, source_addr,
                                        ).await;
                                    }
                                    Err(error) => {
                                        log_listener_connection_error(
                                            "shadowsocks", &tag, &remote_addr, &error,
                                        );
                                    }
                                }
                                Ok(())
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "shadowsocks: accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    match result {
                        Some(Err(error)) if !error.is_cancelled() => {
                            error!(error = %error, "shadowsocks connection task panicked");
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
                    error!(error = %error, "shadowsocks shutdown error");
                }
            }
        }

        info!(inbound_tag = %inbound.tag, protocol = "shadowsocks", "listener stopped");
        Ok(())
    }
}

// ── AEAD relay helpers ─────────────────────────────────────────────────

async fn ss_decrypt_upload(
    mut client: impl AsyncRead + Unpin + Send + 'static,
    mut upstream: impl AsyncWrite + Unpin + Send + 'static,
    cipher: CipherKind,
    key: Vec<u8>,
    next_nonce: u64,
    rate_bps: Option<u64>,
    mut on_bytes: impl FnMut(u64) + Send + 'static,
) -> Result<(), ()> {
    use shadowsocks::{decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload};

    let mut limiter = rate_bps.filter(|b| *b > 0).map(RateLimiter::new);
    let mut nonce = next_nonce;
    loop {
        let mut encrypted_length = vec![0u8; 2 + cipher.tag_len()];
        if client.read_exact(&mut encrypted_length).await.is_err() {
            break;
        }
        let Ok(payload_len) = decrypt_tcp_chunk_length(cipher, &key, &mut nonce, &encrypted_length)
        else {
            break;
        };

        let mut encrypted_payload = vec![0u8; payload_len + cipher.tag_len()];
        if client.read_exact(&mut encrypted_payload).await.is_err() {
            break;
        }
        match decrypt_tcp_chunk_payload(cipher, &key, &mut nonce, payload_len, &encrypted_payload) {
            Ok(plain) => {
                // Wait for rate-limit tokens before writing (kernel GCRA).
                if let Some(ref mut lim) = limiter {
                    while let Err(wait) = lim.check_n(plain.len() as u64) {
                        tokio::time::sleep(wait).await;
                    }
                }
                if upstream.write_all(&plain).await.is_err() {
                    break;
                }
                on_bytes(plain.len() as u64);
            }
            Err(_) => break,
        }
    }
    let _ = upstream.shutdown().await;
    Ok(())
}

async fn ss_encrypt_download(
    mut upstream: impl AsyncRead + Unpin,
    mut client: impl AsyncWrite + Unpin,
    cipher: CipherKind,
    key: Vec<u8>,
    rate_bps: Option<u64>,
    mut on_bytes: impl FnMut(u64),
) -> Result<(), ()> {
    let mut limiter = rate_bps.filter(|b| *b > 0).map(RateLimiter::new);
    let mut nonce: u64 = 0;
    let mut buf = [0u8; 16384];
    loop {
        match upstream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => match ShadowsocksInbound::encrypt_chunk(cipher, &key, &mut nonce, &buf[..n]) {
                Ok(encrypted) => {
                    // Wait for rate-limit tokens before writing (kernel GCRA).
                    if let Some(ref mut lim) = limiter {
                        while let Err(wait) = lim.check_n(encrypted.len() as u64) {
                            tokio::time::sleep(wait).await;
                        }
                    }
                    if client.write_all(&encrypted).await.is_err() {
                        break;
                    }
                    on_bytes(n as u64);
                }
                Err(_) => break,
            },
            Err(_) => break,
        }
    }
    let _ = client.shutdown().await;
    Ok(())
}

// ── UDP relay (uses generic UdpDispatch for routing) ────────────────

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError> {
        use zero_core::ProtocolType;

        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        // Map session_id -> client_addr for response delivery.
        let mut client_sessions: std::collections::HashMap<u64, SocketAddr> =
            std::collections::HashMap::new();

        let mut buf = [0u8; 65536];
        let mut direct_buf = [0u8; 65536];

        loop {
            let (direct_sock, chain_tasks) = dispatch.poll_sockets();

            tokio::select! {
                recv = udp_socket.recv_from(&mut buf) => {
                    let (n, client_addr) = match recv {
                        Ok(r) => r,
                        Err(e) => { warn!(error = %e, "ss udp recv error"); break Ok(()); }
                    };
                    let packet = &buf[..n];

                    let salt_len = cipher.salt_len();
                    if packet.len() < salt_len + cipher.tag_len() { continue; }
                    let salt = &packet[..salt_len];
                    let encrypted = &packet[salt_len..];

                    let Ok(key) = ss_derive_key(cipher, password.as_bytes(), salt) else { continue };
                    let Ok(plain) = aead_decrypt_udp(cipher, &key, &[0u8; 12], encrypted) else { continue };
                    let Ok((target, port, payload_offset)) = parse_target_data(&plain) else { continue };
                    let payload = &plain[payload_offset..];

                    let mut sa = zero_core::SessionAuth::new("shadowsocks");
                    sa.principal_key = Some(password.to_owned());
                    match dispatch.dispatch(
                        self, target, port, payload,
                        ProtocolType::Shadowsocks, Some(&sa),
                    ).await {
                        Ok(session_id) => {
                            client_sessions.insert(session_id, client_addr);
                        }
                        Err(error) => {
                            warn!(error = %error, "ss udp dispatch failed");
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                        if let Some(&client) = client_sessions.get(&sid) {
                            ss_send_encrypted(
                                udp_socket.as_ref(), cipher, password,
                                &direct_buf[..n], client,
                            );
                        }
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                if let Some(&client) = client_sessions.get(&sid) {
                                    let Ok(td) = build_target_data(&target, port, &payload) else { continue };
                                    ss_send_encrypted(
                                        udp_socket.as_ref(), cipher, password, &td, client,
                                    );
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "ss chain response error");
                        }
                        Err(e) => {
                            warn!(error = %e, "ss chain task panicked");
                        }
                    }
                }
            }
        }
    }
}

/// Encrypt plaintext with SS AEAD and send via socket.
fn ss_send_encrypted(
    socket: &UdpSocket,
    cipher: CipherKind,
    password: &str,
    plain: &[u8],
    client: SocketAddr,
) {
    use ring::rand::SecureRandom;
    let mut salt = vec![0u8; cipher.salt_len()];
    let _ = ring::rand::SystemRandom::new().fill(&mut salt);
    let Ok(key) = ss_derive_key(cipher, password.as_bytes(), &salt) else {
        return;
    };
    let Ok(encrypted) = aead_encrypt_udp(cipher, &key, &[0u8; 12], plain) else {
        return;
    };
    let mut resp = salt;
    resp.extend_from_slice(&encrypted);
    let _ = tokio::runtime::Handle::try_current().and_then(|rt| {
        Ok(rt.block_on(async {
            let _ = socket.send_to(&resp, client).await;
        }))
    });
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

fn addr_to_socket(addr: &Address, port: u16) -> Option<SocketAddr> {
    match addr {
        Address::Ipv4(b) => Some(SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3])),
            port,
        )),
        Address::Ipv6(b) => Some(SocketAddr::new(std::net::IpAddr::V6((*b).into()), port)),
        Address::Domain(_) => None,
    }
}

fn ss_derive_key(
    cipher: CipherKind,
    password: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    use shadowsocks::derive_key;
    if cipher.is_blake3() {
        shadowsocks::derive_key_blake3(password, salt, cipher.key_len())
    } else {
        derive_key(password, salt, cipher.key_len())
    }
}

fn download_key_for_client(
    cipher: CipherKind,
    password: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    ss_derive_key(cipher, password, salt)
}

async fn resolve_socket_addr(
    addr: &Address,
    port: u16,
    resolver: &impl zero_traits::DnsResolver,
) -> Option<SocketAddr> {
    match addr {
        Address::Ipv4(b) => Some(SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3])),
            port,
        )),
        Address::Ipv6(b) => Some(SocketAddr::new(std::net::IpAddr::V6((*b).into()), port)),
        Address::Domain(domain) => {
            let ips = resolver.resolve(domain).await.ok()?;
            let ip = ips.first()?;
            let addr = match ip {
                zero_traits::IpAddress::V4(b) => SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3])),
                    port,
                ),
                zero_traits::IpAddress::V6(b) => {
                    SocketAddr::new(std::net::IpAddr::V6((*b).into()), port)
                }
            };
            Some(addr)
        }
    }
}
