//! Shadowsocks UDP outbound — encrypt + send, with response relay.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock, Mutex};

use tokio::net::UdpSocket;
use zero_core::Address;
use zero_engine::EngineError;
use zero_protocol_shadowsocks::{
    aead_decrypt_udp, aead_encrypt_udp, build_target_data, derive_key, parse_target_data,
    CipherKind,
};

// ── types ─────────────────────────────────────────────────────────────

/// A decrypted response from a Shadowsocks upstream.
#[derive(Debug, Clone)]
pub struct SsDecrypted {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

struct SsUpstream {
    socket: Arc<UdpSocket>,
    cipher: CipherKind,
    password: String,
    /// Pending decrypted responses, drained by callers via `drain_responses`.
    pending: Mutex<VecDeque<SsDecrypted>>,
}

static SS_UPSTREAMS: LazyLock<Mutex<HashMap<(String, u16), Arc<SsUpstream>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── send ──────────────────────────────────────────────────────────────

pub async fn send_ss_udp_packet(
    server: &str,
    port: u16,
    password: &str,
    cipher: &str,
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let cipher_kind = CipherKind::from_str(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown shadowsocks cipher: {cipher}"),
        ))
    })?;

    let target_data = build_target_data(target, target_port, payload)
        .map_err(|e| EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

    let mut salt = vec![0u8; cipher_kind.salt_len()];
    use ring::rand::SecureRandom;
    ring::rand::SystemRandom::new()
        .fill(&mut salt)
        .map_err(|_| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ss: random failed",
            ))
        })?;

    let key = derive_key(password.as_bytes(), &salt, cipher_kind.key_len())
        .map_err(|e| EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

    let nonce = [0u8; 12];
    let encrypted = aead_encrypt_udp(cipher_kind, &key, &nonce, &target_data)
        .map_err(|e| EngineError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    let upstream = ensure_upstream(server, port, password, cipher_kind);

    let target_addr: SocketAddr = format!("{server}:{port}").parse().map_err(|_| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid ss upstream address: {server}:{port}"),
        ))
    })?;

    let mut packet = salt;
    packet.extend_from_slice(&encrypted);

    let sent = upstream
        .socket
        .send_to(&packet, target_addr)
        .await
        .map_err(EngineError::from)?;

    Ok(sent)
}

// ── response polling ──────────────────────────────────────────────────

/// Drain all pending decrypted responses from ALL SS upstreams.
pub fn drain_all_responses() -> Vec<SsDecrypted> {
    let upstreams = SS_UPSTREAMS.lock().expect("ss upstream lock poisoned");
    let mut all = Vec::new();
    for up in upstreams.values() {
        all.extend(
            up.pending
                .lock()
                .expect("ss pending lock poisoned")
                .drain(..),
        );
    }
    all
}

// ── internal ──────────────────────────────────────────────────────────

fn ensure_upstream(
    server: &str,
    port: u16,
    password: &str,
    cipher_kind: CipherKind,
) -> Arc<SsUpstream> {
    let key = (server.to_owned(), port);
    let mut upstreams = SS_UPSTREAMS.lock().expect("ss upstream lock poisoned");
    if let Some(existing) = upstreams.get(&key) {
        return existing.clone();
    }

    let socket = {
        let sock = UdpSocket::from_std(
            std::net::UdpSocket::bind("0.0.0.0:0").expect("ss: failed to bind outbound UDP socket"),
        )
        .expect("ss: failed to create tokio UDP socket");
        Arc::new(sock)
    };

    let upstream = Arc::new(SsUpstream {
        socket: socket.clone(),
        cipher: cipher_kind,
        password: password.to_owned(),
        pending: Mutex::new(VecDeque::new()),
    });

    upstreams.insert(key.clone(), upstream.clone());

    // Spawn background recv relay once per upstream.
    tokio::spawn(recv_relay(upstream.clone()));

    upstream
}

async fn recv_relay(upstream: Arc<SsUpstream>) {
    let mut buf = vec![0u8; 4096];
    loop {
        let (n, _from) = match upstream.socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(_) => break,
        };
        let packet = &buf[..n];

        let salt_len = upstream.cipher.salt_len();
        let tag_len = upstream.cipher.tag_len();
        if packet.len() < salt_len + tag_len {
            continue;
        }

        let Ok(key) = derive_key(
            upstream.password.as_bytes(),
            &packet[..salt_len],
            upstream.cipher.key_len(),
        ) else {
            continue;
        };
        let nonce = [0u8; 12];
        let Ok(plain) = aead_decrypt_udp(upstream.cipher, &key, &nonce, &packet[salt_len..]) else {
            continue;
        };
        let Ok((target, target_port, payload_offset)) = parse_target_data(&plain) else {
            continue;
        };

        upstream
            .pending
            .lock()
            .expect("ss pending lock poisoned")
            .push_back(SsDecrypted {
                target,
                port: target_port,
                payload: plain[payload_offset..].to_vec(),
            });
    }
}
