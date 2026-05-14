// Shadowsocks inbound — shadowsocks.rs

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::Address;
use zero_engine::EngineError;
use zero_protocol_shadowsocks::{CipherKind, ShadowsocksInbound};

use crate::runtime::{bind_listener, Proxy};
use crate::transport::{MeteredStream, TcpRelayStream};

use super::super::logging::log_listener_connection_error;

impl Proxy {
    pub(crate) async fn run_shadowsocks_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (password, cipher_str) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Shadowsocks { password, cipher } => {
                (password.clone(), cipher.clone())
            }
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

        // Bind UDP socket on same port for SS UDP relay
        let udp_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        let udp_socket = match UdpSocket::bind(&udp_addr).await {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                warn!(error = %e, "shadowsocks: failed to bind UDP socket, UDP disabled");
                None
            }
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

        // Spawn UDP relay task
        if let Some(udp) = udp_socket.as_ref() {
            let udp = udp.clone();
            let engine = self.clone();
            let password = password.clone();
            let inbound_tag_clone = inbound.tag.clone();
            connections.spawn(async move {
                engine.ss_udp_relay_loop(udp, &inbound_tag_clone, &password, cipher).await
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
                    let (stream, remote_addr) = accept_result?;
                    let engine = self.clone();
                    let inbound_tag = inbound.tag.clone();
                    let password = password.clone();

                    connections.spawn(async move {
                        if let Err(error) = engine.handle_shadowsocks_connection(
                            stream, inbound_tag.as_str(), &password, cipher,
                        ).await {
                            log_listener_connection_error(
                                "shadowsocks",
                                inbound_tag.as_str(),
                                &remote_addr,
                                &error,
                            );
                        }
                        Ok(())
                    });
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "shadowsocks connection task panicked");
                        }
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

    /// SS UDP relay loop: decrypt incoming packets, route (direct or SS chain), encrypt response.
    async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError> {
        use zero_core::{Network, ProtocolType, Session};
        use zero_protocol_shadowsocks::{
            aead_decrypt_udp, aead_encrypt_udp, build_target_data, parse_target_data,
        };

        let mut buf = [0u8; 65536];
        let relay_socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
            EngineError::Io(io::Error::other(format!("ss udp relay socket: {e}")))
        })?;

        // Direct sessions: client_addr → (target_addr, port)
        let mut direct_sessions: std::collections::HashMap<SocketAddr, (Address, u16)> =
            std::collections::HashMap::new();

        // Chained SS upstream connections: by (upstream_server, port)
        let mut ss_upstreams: std::collections::HashMap<
            (String, u16),
            (std::sync::Arc<UdpSocket>, CipherKind, String),
        > = std::collections::HashMap::new();
        let mut chained_clients: std::collections::HashMap<SocketAddr, (String, u16, Address, u16)> =
            std::collections::HashMap::new();

        let mut recv_buf = [0u8; 65536];
        let mut chain_buf = [0u8; 65536];
        loop {
            select! {
                // Incoming SS UDP packet from client
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

                        // Route decision
                        let mut session = Session::new(0, target.clone(), port, Network::Udp, ProtocolType::Shadowsocks);
                        self.prepare_session(&mut session, inbound_tag);
                        let action = self.route_decision(&session.target);
                        let Ok(resolved) = self.resolve_outbound(&action) else { continue };

                        // Check if resolved to SS chain outbound
                        let leaf = match &resolved {
                            zero_engine::ResolvedOutbound::Single(c) => c,
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
                            // SS chain: encrypt and forward to upstream SS server
                            let chain_key = (server.to_string(), upstream_port);
                            let chain_cipher = CipherKind::from_str(&chain_cipher_str);
                            let Some(chain_cipher) = chain_cipher else { continue };

                            let upstream_entry = ss_upstreams
                                .entry(chain_key.clone())
                                .or_insert_with(|| {
                                    let sock = std::sync::Arc::new(
                                        tokio::net::UdpSocket::from_std(
                                            std::net::UdpSocket::bind("0.0.0.0:0").unwrap()
                                        ).unwrap()
                                    );
                                    (sock, chain_cipher, chain_pwd.to_string())
                                });
                            let upstream = upstream_entry.0.clone();
                            let upstream = upstream.clone();

                            // Track client for response routing
                            chained_clients.insert(client_addr, (server.to_string(), upstream_port, target.clone(), port));

                            // Encrypt for upstream: salt + aead_encrypt_udp([target][port][payload])
                            let target_data = match build_target_data(&target, port, payload) {
                                Ok(d) => d,
                                Err(_) => continue,
                            };
                            let mut up_salt = vec![0u8; chain_cipher.salt_len()];
                            use ring::rand::SecureRandom;
                            let _ = ring::rand::SystemRandom::new().fill(&mut up_salt);
                            let Ok(up_key) = ss_derive_key(chain_cipher, chain_pwd.as_bytes(), &up_salt) else { continue };
                            let Ok(up_encrypted) = aead_encrypt_udp(chain_cipher, &up_key, &nonce, &target_data) else { continue };

                            let target_addr = format!("{server}:{upstream_port}");
                            if let Ok(addr) = target_addr.parse::<std::net::SocketAddr>() {
                                let mut packet = up_salt;
                                packet.extend_from_slice(&up_encrypted);
                                let _ = upstream.send_to(&packet, addr).await;
                            }
                        } else {
                            // Direct: forward to target
                            let target_addr = resolve_socket_addr(&target, port, &self.resolver).await;
                            let Some(target_addr) = target_addr else { continue };
                            direct_sessions.insert(client_addr, (target.clone(), port));
                            let _ = relay_socket.send_to(payload, target_addr).await;
                        }
                    }
                }

                // Response from direct target
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
                    use ring::rand::SecureRandom;
                    let _ = ring::rand::SystemRandom::new().fill(&mut salt);
                    let Ok(key) = ss_derive_key(cipher, password.as_bytes(), &salt) else { continue };
                    let nonce = [0u8; 12];
                    let Ok(encrypted) = aead_encrypt_udp(cipher, &key, &nonce, &recv_buf[..n]) else { continue };

                    let mut resp = salt;
                    resp.extend_from_slice(&encrypted);
                    let _ = udp_socket.send_to(&resp, client_addr).await;
                }

                // Response from chained SS upstream
                _ = Self::ss_chain_recv_any(&ss_upstreams, &mut chained_clients, udp_socket.as_ref(), cipher, password, &mut chain_buf) => {}
            }
        }
        Ok(())
    }

    /// Poll all chained SS upstream sockets for responses, decrypt, and forward to clients.
    async fn ss_chain_recv_any(
        upstreams: &std::collections::HashMap<(String, u16), (std::sync::Arc<UdpSocket>, CipherKind, String)>,
        clients: &mut std::collections::HashMap<SocketAddr, (String, u16, Address, u16)>,
        out_socket: &UdpSocket,
        inbound_cipher: CipherKind,
        inbound_password: &str,
        buf: &mut [u8],
    ) {
        for (sock, chain_cipher, chain_pwd) in upstreams.values() {
            let Ok(n) = sock.try_recv(buf) else { continue };
            let salt_len = chain_cipher.salt_len();
            if n < salt_len + chain_cipher.tag_len() { continue; }
            let salt = &buf[..salt_len];
            let encrypted = &buf[salt_len..n];

            let Ok(key) = ss_derive_key(*chain_cipher, chain_pwd.as_bytes(), salt) else { continue };
            let nonce = [0u8; 12];
            let Ok(plain) = zero_protocol_shadowsocks::aead_decrypt_udp(*chain_cipher, &key, &nonce, encrypted) else { continue };
            let Ok((_, _, payload_offset)) = zero_protocol_shadowsocks::parse_target_data(&plain) else { continue };
            let payload = &plain[payload_offset..];

            // Find the client for this response (simplified: forward to first matching client)
            let client_addr = clients.iter()
                .find(|(_, (s, p, _, _))| {
                    upstreams.contains_key(&(s.clone(), *p))
                })
                .map(|(c, _)| *c);
            let Some(client_addr) = client_addr else { continue };

            let mut resp_salt = vec![0u8; inbound_cipher.salt_len()];
            use ring::rand::SecureRandom;
            let _ = ring::rand::SystemRandom::new().fill(&mut resp_salt);
            let Ok(resp_key) = ss_derive_key(inbound_cipher, inbound_password.as_bytes(), &resp_salt) else { continue };
            let Ok(resp_enc) = zero_protocol_shadowsocks::aead_encrypt_udp(inbound_cipher, &resp_key, &nonce, payload) else { continue };

            let mut resp = resp_salt;
            resp.extend_from_slice(&resp_enc);
            let _ = out_socket.send_to(&resp, client_addr).await;
            return;
        }
    }

    async fn handle_shadowsocks_connection<S>(
        &self,
        client: S,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError>
    where
        S: crate::transport::ClientStream + Send + 'static,
    {
        let mut metered = MeteredStream::new(client);
        let accept = ShadowsocksInbound
            .accept_request(&mut metered, cipher, password.as_bytes())
            .await?;

        let mut session = accept.session;
        self.record_session_inbound_traffic(session.id, metered.drain_traffic());

        // Route
        self.prepare_session(&mut session, inbound_tag);
        let action = self.route_decision(&session.target);
        let Ok(resolved) = self.resolve_outbound(&action) else {
            return Ok(());
        };

        let upstream = match self.establish_tcp_outbound(&session, resolved).await {
            Ok(outbound) => match outbound {
                crate::transport::EstablishedTcpOutbound::Direct { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Vless { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Socks5 { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Hysteria2 { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Shadowsocks { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Block { .. } => return Ok(()),
            },
            Err(_e) => {
                error!("shadowsocks tcp outbound failed");
                return Ok(());
            }
        };

        // Split both streams for bidirectional relay
        let (client_read, client_write) = tokio::io::split(metered);
        let (up_read, mut up_write) = tokio::io::split(upstream);

        // Write remaining payload from first chunk to upstream
        if !accept.remaining_payload.is_empty() {
            up_write.write_all(&accept.remaining_payload).await.ok();
        }

        // AEAD relay: two concurrent tasks
        let key = accept.session_key;
        let key_up = key.clone();
        let key_down = key;

        let upload = tokio::spawn(ss_decrypt_upload(client_read, up_write, cipher, key_up));
        let download = tokio::spawn(ss_encrypt_download(up_read, client_write, cipher, key_down));

        let _ = tokio::try_join!(upload, download);
        Ok(())
    }
}

/// Decrypt relay: client → server (upload direction).
/// Reads AEAD chunks from client, decrypts, writes plaintext to upstream.
async fn ss_decrypt_upload(
    mut client: impl AsyncRead + Unpin + Send + 'static,
    mut upstream: impl AsyncWrite + Unpin + Send + 'static,
    cipher: CipherKind,
    key: Vec<u8>,
) -> Result<(), ()> {
    let mut nonce: u64 = 1; // nonce 0 was the first chunk (handled in accept)
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

/// Encrypt relay: server → client (download direction).
/// Reads plaintext from upstream, encrypts, writes AEAD chunks to client.
async fn ss_encrypt_download(
    mut upstream: impl AsyncRead + Unpin,
    mut client: impl AsyncWrite + Unpin,
    cipher: CipherKind,
    key: Vec<u8>,
) -> Result<(), ()> {
    let mut nonce: u64 = 0;
    let mut buf = [0u8; 16384];
    loop {
        match upstream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                match ShadowsocksInbound::encrypt_chunk(cipher, &key, &mut nonce, &buf[..n]) {
                    Ok(encrypted) => {
                        if client.write_all(&encrypted).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            Err(_) => break,
        }
    }
    let _ = client.shutdown().await;
    Ok(())
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

fn ss_derive_key(cipher: CipherKind, password: &[u8], salt: &[u8]) -> Result<Vec<u8>, zero_core::Error> {
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
    use zero_traits::DnsResolver;
    match addr {
        Address::Ipv4(b) => Some(SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3])),
            port,
        )),
        Address::Ipv6(b) => Some(SocketAddr::new(
            std::net::IpAddr::V6((*b).into()),
            port,
        )),
        Address::Domain(domain) => {
            let ips = resolver.resolve(domain).await.ok()?;
            let ip = ips.first()?;
            let addr = match ip {
                zero_traits::IpAddress::V4(b) => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3])), port),
                zero_traits::IpAddress::V6(b) => SocketAddr::new(std::net::IpAddr::V6((*b).into()), port),
            };
            Some(addr)
        }
    }
}
