//! Shadowsocks inbound — AEAD accept, route, AEAD-framed relay.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use ring::rand::SecureRandom;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_protocol_shadowsocks::{
    aead_decrypt_udp, aead_encrypt_udp, build_target_data, parse_target_data, CipherKind,
    ShadowsocksInbound,
};

use crate::logging::log_listener_connection_error;
use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, RateLimiter, TcpRelayStream};

// ── Client stream wrapper (carries AEAD key + first-chunk payload) ────

pub(crate) struct SsClientStream {
    inner: TcpRelayStream,
    key: Vec<u8>,
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

        Ok((
            session,
            SsClientStream {
                inner: metered.into_inner(),
                key: accept.session_key,
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
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        let (client_read, client_write) = tokio::io::split(client.inner);
        let (up_read, mut up_write) = tokio::io::split(upstream);

        if !client.remaining.is_empty() {
            let _ = up_write.write_all(&client.remaining).await;
        }

        let key_up = client.key.clone();
        let key_down = client.key;
        let cipher = self.cipher;
        let upload = tokio::spawn(async move {
            let _ = ss_decrypt_upload(client_read, up_write, cipher, key_up, up_bps).await;
        });
        let download = tokio::spawn(async move {
            let _ = ss_encrypt_download(up_read, client_write, cipher, key_down, down_bps).await;
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
                password, cipher, up_bps, down_bps,
            } => (password.clone(), cipher.clone(), *up_bps, *down_bps),
            _ => return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "shadowsocks listener requires shadowsocks config",
            ))),
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
            "{}:{}", inbound.listen.address, inbound.listen.port
        )).await {
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
            connections.spawn(async move {
                engine.ss_udp_relay_loop(udp, &tag, &password, cipher).await
            });
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
    rate_bps: Option<u64>,
) -> Result<(), ()> {
    let mut limiter = rate_bps
        .filter(|b| *b > 0)
        .map(|b| RateLimiter::new(b));
    let mut nonce: u64 = 1;
    let mut len_buf = [0u8; 2];
    loop {
        if client.read_exact(&mut len_buf).await.is_err() {
            break;
        }
        let chunk_len = u16::from_be_bytes(len_buf) as usize;
        if chunk_len < cipher.tag_len() || chunk_len > 65535 {
            break;
        }
        let mut chunk = vec![0u8; chunk_len];
        if client.read_exact(&mut chunk).await.is_err() {
            break;
        }
        match ShadowsocksInbound::decrypt_chunk(cipher, &key, &mut nonce, &chunk) {
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
) -> Result<(), ()> {
    let mut limiter = rate_bps
        .filter(|b| *b > 0)
        .map(|b| RateLimiter::new(b));
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
                }
                Err(_) => break,
            },
            Err(_) => break,
        }
    }
    let _ = client.shutdown().await;
    Ok(())
}

// ── UDP relay (kept as Proxy method for now) ────────────────────────────

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError> {
        use zero_core::{Network, ProtocolType};

        let mut buf = [0u8; 65536];
        let relay_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("ss udp relay socket: {e}"))))?;

        let mut direct_sessions: std::collections::HashMap<SocketAddr, (Address, u16)> =
            std::collections::HashMap::new();

        let mut ss_upstreams: std::collections::HashMap<
            (String, u16),
            (Arc<UdpSocket>, CipherKind, String),
        > = std::collections::HashMap::new();
        let mut chained_clients: std::collections::HashMap<
            SocketAddr,
            (String, u16, Address, u16),
        > = std::collections::HashMap::new();

        let mut recv_buf = [0u8; 65536];
        let mut chain_buf = [0u8; 65536];

        loop {
            select! {
                recv = udp_socket.recv_from(&mut buf) => {
                    let (n, client_addr) = match recv {
                        Ok(r) => r,
                        Err(e) => { warn!(error = %e, "ss udp recv error"); break; }
                    };
                    let packet = &buf[..n];

                    let salt_len = cipher.salt_len();
                    if packet.len() < salt_len + cipher.tag_len() { continue; }
                    let salt = &packet[..salt_len];
                    let encrypted = &packet[salt_len..];

                    let Ok(key) = ss_derive_key(cipher, password.as_bytes(), salt) else { continue };
                    let nonce = [0u8; 12];
                    let Ok(plain) = aead_decrypt_udp(cipher, &key, &nonce, encrypted) else { continue };

                    if let Ok((target, port, payload_offset)) = parse_target_data(&plain) {
                        let payload = &plain[payload_offset..];

                        let mut session = Session::new(0, target.clone(), port, Network::Udp, ProtocolType::Shadowsocks);
                        let mut sa = zero_core::SessionAuth::new("shadowsocks");
                        sa.principal_key = Some(password.to_owned());
                        session.auth = Some(sa);
                        self.prepare_session(&mut session, inbound_tag, None);
                        self.resolve_fake_ip_target(&mut session).await;
                        let action = self.route_decision(&session);
                        let Ok((resolved, _plan)) = self.resolve_outbound(&action) else { continue };

                        let leaf = match &resolved {
                            zero_engine::ResolvedOutbound::Single(c) => c,
                            zero_engine::ResolvedOutbound::Relay { .. } => continue,
                            zero_engine::ResolvedOutbound::Fallback { candidates } => {
                                let Some(first) = candidates.first() else { continue };
                                first
                            }
                        };
                        let ss_chain = match leaf {
                            zero_engine::ResolvedLeafOutbound::Shadowsocks { server, port, password: chain_pwd, cipher: chain_cipher, .. } =>
                                Some((server.to_owned(), *port, chain_pwd.to_owned(), chain_cipher.to_owned())),
                            _ => None,
                        };

                        if let Some((server, upstream_port, chain_pwd, chain_cipher_str)) = ss_chain {
                            let chain_key = (server.to_string(), upstream_port);
                            let chain_cipher = CipherKind::from_str(&chain_cipher_str);
                            let Some(chain_cipher) = chain_cipher else { continue };

                            let upstream_entry = ss_upstreams
                                .entry(chain_key.clone())
                                .or_insert_with(|| {
                                    let sock = Arc::new(
                                        tokio::net::UdpSocket::from_std(
                                            std::net::UdpSocket::bind("0.0.0.0:0").unwrap()
                                        ).unwrap()
                                    );
                                    (sock, chain_cipher, chain_pwd.to_string())
                                });
                            let upstream = upstream_entry.0.clone();
                            chained_clients.insert(client_addr, (server.to_string(), upstream_port, target.clone(), port));

                            let target_data = match build_target_data(&target, port, payload) {
                                Ok(d) => d,
                                Err(_) => continue,
                            };
                            let mut up_salt = vec![0u8; chain_cipher.salt_len()];
                            let _ = ring::rand::SystemRandom::new().fill(&mut up_salt);
                            let Ok(up_key) = ss_derive_key(chain_cipher, chain_pwd.as_bytes(), &up_salt) else { continue };
                            let Ok(up_encrypted) = aead_encrypt_udp(chain_cipher, &up_key, &nonce, &target_data) else { continue };

                            let target_addr = format!("{server}:{upstream_port}");
                            if let Ok(addr) = target_addr.parse::<SocketAddr>() {
                                let mut packet = up_salt;
                                packet.extend_from_slice(&up_encrypted);
                                let _ = upstream.send_to(&packet, addr).await;
                            }
                        } else {
                            let target_addr = resolve_socket_addr(&target, port, self.resolver.as_ref()).await;
                            let Some(target_addr) = target_addr else { continue };
                            direct_sessions.insert(client_addr, (target.clone(), port));
                            let _ = relay_socket.send_to(payload, target_addr).await;
                        }
                    }
                }

                recv = relay_socket.recv_from(&mut recv_buf) => {
                    let (n, target_addr) = match recv {
                        Ok(r) => r,
                        Err(e) => { warn!(error = %e, "ss udp relay recv error"); break; }
                    };

                    let client_addr = direct_sessions.iter()
                        .find(|(_, (t, p))| addr_to_socket(t, *p) == Some(target_addr))
                        .map(|(c, _)| *c);

                    let Some(client_addr) = client_addr else { continue };

                    let mut salt = vec![0u8; cipher.salt_len()];
                    let _ = ring::rand::SystemRandom::new().fill(&mut salt);
                    let Ok(key) = ss_derive_key(cipher, password.as_bytes(), &salt) else { continue };
                    let nonce = [0u8; 12];
                    let Ok(encrypted) = aead_encrypt_udp(cipher, &key, &nonce, &recv_buf[..n]) else { continue };

                    let mut resp = salt;
                    resp.extend_from_slice(&encrypted);
                    let _ = udp_socket.send_to(&resp, client_addr).await;
                }

                _ = Self::ss_chain_recv_any(&ss_upstreams, &mut chained_clients, udp_socket.as_ref(), cipher, password, &mut chain_buf) => {}
            }
        }
        Ok(())
    }

    async fn ss_chain_recv_any(
        upstreams: &std::collections::HashMap<
            (String, u16),
            (Arc<UdpSocket>, CipherKind, String),
        >,
        clients: &mut std::collections::HashMap<SocketAddr, (String, u16, Address, u16)>,
        out_socket: &UdpSocket,
        inbound_cipher: CipherKind,
        inbound_password: &str,
        buf: &mut [u8],
    ) {
        for (sock, chain_cipher, chain_pwd) in upstreams.values() {
            let Ok(n) = sock.try_recv(buf) else { continue };
            let salt_len = chain_cipher.salt_len();
            if n < salt_len + chain_cipher.tag_len() {
                continue;
            }
            let salt = &buf[..salt_len];
            let encrypted = &buf[salt_len..n];

            let Ok(key) = ss_derive_key(*chain_cipher, chain_pwd.as_bytes(), salt) else { continue };
            let nonce = [0u8; 12];
            let Ok(plain) = zero_protocol_shadowsocks::aead_decrypt_udp(*chain_cipher, &key, &nonce, encrypted) else { continue };
            let Ok((_, _, payload_offset)) = zero_protocol_shadowsocks::parse_target_data(&plain) else { continue };
            let payload = &plain[payload_offset..];

            let client_addr = clients
                .iter()
                .find(|(_, (s, p, _, _))| upstreams.contains_key(&(s.clone(), *p)))
                .map(|(c, _)| *c);
            let Some(client_addr) = client_addr else { continue };

            let mut resp_salt = vec![0u8; inbound_cipher.salt_len()];
            let _ = ring::rand::SystemRandom::new().fill(&mut resp_salt);
            let Ok(resp_key) = ss_derive_key(inbound_cipher, inbound_password.as_bytes(), &resp_salt) else { continue };
            let Ok(resp_enc) = zero_protocol_shadowsocks::aead_encrypt_udp(
                inbound_cipher, &resp_key, &nonce, payload,
            ) else { continue };

            let mut resp = resp_salt;
            resp.extend_from_slice(&resp_enc);
            let _ = out_socket.send_to(&resp, client_addr).await;
            return;
        }
    }
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<SocketAddr> {
    addr.and_then(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => Some(SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)),
            0,
        )),
        zero_traits::IpAddress::V6(octets) => Some(SocketAddr::new(
            std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)),
            0,
        )),
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
    use zero_protocol_shadowsocks::derive_key;
    if cipher.is_blake3() {
        zero_protocol_shadowsocks::derive_key_blake3(password, salt, cipher.key_len())
    } else {
        derive_key(password, salt, cipher.key_len())
    }
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
