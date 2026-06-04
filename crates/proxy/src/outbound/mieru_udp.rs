//! Mieru UDP outbound — connect, handshake, relay encrypted UDP datagrams.
//!
//! Mieru tunnels SOCKS5 UDP packets over TCP with XChaCha20-Poly1305 encryption.
//! Each datagram is wrapped with Mieru's UDP associate framing then encrypted
//! as a DATA_CLIENT_TO_SERVER segment.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

use mieru::{unwrap_udp_associate, wrap_udp_associate, MieruOutbound};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::warn;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

// ── Types ───────────────────────────────────────────────────────────

/// A decrypted, unwrapped response from a Mieru upstream.
#[derive(Debug, Clone)]
pub struct MieruUdpResponse {
    pub payload: Vec<u8>,
}

type MieruUpstreamKey = (String, u16); // (server, port)

struct MieruCachedUpstream {
    send_tx: mpsc::Sender<Vec<u8>>,
}

// ── Global state ────────────────────────────────────────────────────

static MIERU_PENDING: LazyLock<Arc<Mutex<VecDeque<MieruUdpResponse>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(VecDeque::new())));

static MIERU_CACHE: LazyLock<Mutex<HashMap<MieruUpstreamKey, MieruCachedUpstream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── Public API ──────────────────────────────────────────────────────

/// Drain all pending Mieru UDP responses (synchronous).
pub fn drain_all_mieru_responses() -> Vec<MieruUdpResponse> {
    MIERU_PENDING
        .lock()
        .expect("mieru pending lock poisoned")
        .drain(..)
        .collect()
}

/// Send a UDP payload through a Mieru upstream, creating one if needed.
pub async fn send_mieru_udp_packet(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    server_port: u16,
    username: &str,
    password: &str,
    _target: &Address,
    _target_port: u16,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let sent = payload.len();
    let key: MieruUpstreamKey = (server.to_owned(), server_port);

    // Reuse existing upstream if available.
    let cached_tx = {
        let cache = MIERU_CACHE.lock().expect("mieru cache lock poisoned");
        cache.get(&key).map(|u| u.send_tx.clone())
    };
    if let Some(tx) = cached_tx {
        let wrapped = wrap_udp_associate(payload);
        let _ = tx.send(wrapped).await;
        proxy.record_session_outbound_tx(session.id, sent as u64);
        return Ok(sent);
    }

    // Establish new upstream.
    let send_tx =
        establish_mieru_upstream(proxy, server, server_port, username, password, session).await?;

    // Cache for reuse.
    MIERU_CACHE
        .lock()
        .expect("mieru cache lock poisoned")
        .insert(
            key,
            MieruCachedUpstream {
                send_tx: send_tx.clone(),
            },
        );

    // Send initial payload.
    let wrapped = wrap_udp_associate(payload);
    let _ = send_tx.send(wrapped).await;
    proxy.record_session_outbound_tx(session.id, sent as u64);
    Ok(sent)
}

// ── Internal ────────────────────────────────────────────────────────

async fn establish_mieru_upstream(
    proxy: &Proxy,
    server: &str,
    port: u16,
    username: &str,
    password: &str,
    session: &Session,
) -> Result<mpsc::Sender<Vec<u8>>, EngineError> {
    // TCP connect
    let socket = proxy
        .protocols
        .direct_outbound
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let mut stream = TcpRelayStream::new(socket);

    // Mieru handshake
    let outbound = MieruOutbound::connect(
        &mut stream,
        username,
        password,
        &session.target,
        session.port,
    )
    .await
    .map_err(|e| EngineError::Io(std::io::Error::other(format!("mieru udp handshake: {e}"))))?;

    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);

    let shared_outbound = Arc::new(tokio::sync::Mutex::new(outbound));
    let shared_stream = Arc::new(tokio::sync::Mutex::new(stream));

    // Send task: wrap, encrypt, write.
    let send_outbound = shared_outbound.clone();
    let send_stream = shared_stream.clone();
    tokio::spawn(async move {
        while let Some(payload) = send_rx.recv().await {
            let mut ob = send_outbound.lock().await;
            match ob.encrypt_client_data(&payload) {
                Ok(encrypted) => {
                    let mut s = send_stream.lock().await;
                    if let Err(e) = AsyncWriteExt::write_all(&mut *s, &encrypted).await {
                        warn!(error = %e, "mieru udp write failed");
                        break;
                    }
                    if let Err(e) = AsyncWriteExt::flush(&mut *s).await {
                        warn!(error = %e, "mieru udp flush failed");
                        break;
                    }
                }
                Err(e) => {
                    warn!(error = %e, "mieru udp encrypt failed");
                    break;
                }
            }
        }
    });

    // Recv task: read, decrypt, unwrap, push to global queue.
    let recv_outbound = shared_outbound.clone();
    let recv_stream = shared_stream.clone();
    tokio::spawn(async move {
        let mut raw = Vec::new();
        loop {
            // Read raw bytes from stream
            let mut scratch = [0u8; 4096];
            let mut s = recv_stream.lock().await;
            match s.read(&mut scratch).await {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&scratch[..n]),
                Err(e) => {
                    warn!(error = %e, "mieru udp read failed");
                    break;
                }
            }

            // Try to decrypt any complete segments
            loop {
                let mut ob = recv_outbound.lock().await;
                match ob.decrypt_server_data_with_consumed(&raw) {
                    Ok((segment, consumed)) => {
                        raw.drain(..consumed);
                        let payload = segment.payload;
                        if !payload.is_empty() {
                            // Unwrap Mieru UDP associate framing
                            if let Ok(unwrapped) = unwrap_udp_associate(&payload) {
                                let mut pending =
                                    MIERU_PENDING.lock().expect("mieru pending lock poisoned");
                                pending.push_back(MieruUdpResponse { payload: unwrapped });
                            }
                        }
                    }
                    Err(error) if error == zero_core::Error::Protocol("mieru: need more data") => {
                        break; // wait for more data
                    }
                    Err(e) => {
                        warn!(error = %e, "mieru udp decrypt failed");
                        return;
                    }
                }
            }
        }
    });

    Ok(send_tx)
}
