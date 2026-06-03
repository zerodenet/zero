//! Hysteria2 outbound protocol implementation.
//!
//! Provides UDP upstream establishment and management for chained
//! Hysteria2 connections, following the same pattern as VLESS.
//!
//! UDP relay is wired into the SOCKS5 UDP associate dispatch path
//! via `send_h2_udp_packet` / `drain_all_h2_responses`.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{error, warn};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_protocol_hysteria2::{build_udp_datagram, parse_udp_datagram};

use crate::runtime::Proxy;
use crate::transport::Hysteria2Connector;

// ── Types ──

/// Handle to an established Hysteria2 UDP upstream connection.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Hysteria2UdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

// ── Relay ──

/// Spawn the bidirectional relay task for a Hysteria2 UDP upstream.
///
/// All payloads sent through this upstream are wrapped in Hysteria2 UDP
/// datagrams addressed to the fixed `target:port`. Responses are unwrapped
/// and pushed to `recv_rx`.
fn spawn_hysteria2_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    conn: Arc<quinn::Connection>,
    h2_sid: u16,
    initial_payload: Vec<u8>,
    target: Address,
    port: u16,
) -> (Hysteria2UdpUpstream, mpsc::Receiver<Vec<u8>>) {
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, recv_rx) = mpsc::channel::<Vec<u8>>(32);

    proxy.record_session_outbound_tx(session_id, initial_payload.len() as u64);

    let conn_send = conn.clone();
    let proxy_send = proxy.clone();
    let target_send = target.clone();
    tokio::spawn(async move {
        let mut pkt_id: u16 = 0;

        // Send initial payload
        if let Ok(dg) = build_udp_datagram(h2_sid, pkt_id, &target_send, port, &initial_payload) {
            if conn_send.send_datagram(dg.into()).is_ok() {
                proxy_send.record_session_outbound_tx(session_id, initial_payload.len() as u64);
            }
        }
        pkt_id = pkt_id.wrapping_add(1);

        // Send subsequent payloads
        while let Some(payload) = send_rx.recv().await {
            let Ok(dg) = build_udp_datagram(h2_sid, pkt_id, &target_send, port, &payload) else {
                break;
            };
            if conn_send.send_datagram(dg.into()).is_err() {
                break;
            }
            proxy_send.record_session_outbound_tx(session_id, payload.len() as u64);
            pkt_id = pkt_id.wrapping_add(1);
        }
    });

    // Receive responses
    let conn_recv = conn.clone();
    tokio::spawn(async move {
        loop {
            match conn_recv.read_datagram().await {
                Ok(data) => {
                    if let Ok(pkt) = parse_udp_datagram(&data) {
                        if recv_tx.send(pkt.payload).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "hysteria2 udp upstream read_datagram error");
                    break;
                }
            }
        }
    });

    (
        Hysteria2UdpUpstream {
            session_id,
            send_tx,
        },
        recv_rx,
    )
}

// ── Global response queue ──────────────────────────────────────────

/// A decrypted response from a Hysteria2 upstream.
#[derive(Debug, Clone)]
pub struct H2Decrypted {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

/// Global pending Hysteria2 UDP responses, drained by the UDP associate loop.
static H2_PENDING: LazyLock<Arc<Mutex<VecDeque<H2Decrypted>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(VecDeque::new())));

/// Drain all pending Hysteria2 UDP responses (synchronous).
pub fn drain_all_h2_responses() -> Vec<H2Decrypted> {
    H2_PENDING
        .lock()
        .expect("h2 pending lock poisoned")
        .drain(..)
        .collect()
}

/// Bridge an async response receiver into the global sync queue.
fn bridge_h2_responses(mut recv_rx: mpsc::Receiver<Vec<u8>>, target: Address, port: u16) {
    tokio::spawn(async move {
        while let Some(payload) = recv_rx.recv().await {
            let mut pending = H2_PENDING.lock().expect("h2 pending lock poisoned");
            pending.push_back(H2Decrypted {
                target: target.clone(),
                port,
                payload,
            });
        }
    });
}

// ── Send (SOCKS5 UDP dispatch) ─────────────────────────────────────

/// Send a UDP payload through a Hysteria2 upstream, reusing existing connections.
///
/// Called by the SOCKS5 UDP associate dispatch path for the first packet
/// to a new target, and for subsequent packets via `forward_existing_udp_flow`.
pub async fn send_h2_udp_packet(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    server_port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let sent = payload.len();
    let key = (server.to_owned(), server_port, target.clone(), target_port);

    // Reuse existing upstream if available.
    let cached_tx = {
        let cache = H2_CACHE.lock().expect("h2 cache lock poisoned");
        cache.get(&key).map(|u| u.send_tx.clone())
    };
    if let Some(tx) = cached_tx {
        let _ = tx.send(payload.to_vec()).await;
        proxy.record_session_outbound_tx(session.id, sent as u64);
        return Ok(sent);
    }

    // Establish new upstream.
    let h2_session = Session::new(
        session.id,
        target.clone(),
        target_port,
        zero_core::Network::Udp,
        zero_core::ProtocolType::Hysteria2,
    );

    match establish_hysteria2_udp_upstream(
        proxy,
        &h2_session,
        server,
        server_port,
        password,
        client_fingerprint,
        payload,
    )
    .await
    {
        Ok((upstream, recv_rx)) => {
            // Cache for reuse.
            let cached = H2CachedUpstream {
                send_tx: upstream.send_tx.clone(),
            };
            H2_CACHE
                .lock()
                .expect("h2 cache lock poisoned")
                .insert(key.clone(), cached);

            // Bridge response receiver to global sync queue.
            bridge_h2_responses(recv_rx, target.clone(), target_port);

            proxy.record_session_outbound_tx(session.id, sent as u64);
            Ok(sent)
        }
        Err(e) => {
            error!(
                %e, server, server_port, ?target, target_port,
                "hysteria2 udp send failed"
            );
            Err(e)
        }
    }
}

// ── Global upstream cache ──────────────────────────────────────────

type H2CacheKey = (String, u16, Address, u16); // (server, port, target, target_port)

#[derive(Clone)]
struct H2CachedUpstream {
    send_tx: mpsc::Sender<Vec<u8>>,
}

/// Global Hysteria2 UDP upstream cache for connection reuse.
static H2_CACHE: LazyLock<Mutex<HashMap<H2CacheKey, H2CachedUpstream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── Establishment ──

/// Establish a Hysteria2 UDP upstream connection.
///
/// Connects to the Hysteria2 server via QUIC, authenticates, and returns
/// a handle for sending/receiving UDP datagrams.
pub async fn establish_hysteria2_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
    initial_payload: &[u8],
) -> Result<(Hysteria2UdpUpstream, mpsc::Receiver<Vec<u8>>), EngineError> {
    let connector =
        Hysteria2Connector::new(server, port, password).with_fingerprint(client_fingerprint);
    let conn = connector.connect_raw().await?;

    let h2_sid: u16 = 1;
    Ok(spawn_hysteria2_udp_relay(
        proxy,
        session.id,
        Arc::new(conn),
        h2_sid,
        initial_payload.to_vec(),
        session.target.clone(),
        session.port,
    ))
}

// ── Manager ──

/// Manages per-target Hysteria2 UDP upstream connections.
/// Kept for future connection-pooling enhancements.
#[allow(dead_code)]
pub struct Hysteria2UdpOutboundManager {
    upstreams: HashMap<(Address, u16), Hysteria2UdpUpstream>,
    response_tasks: JoinSet<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>>,
}

#[allow(dead_code)]
impl Hysteria2UdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
            response_tasks: JoinSet::new(),
        }
    }

    pub fn get(&self, target: &Address, port: u16) -> Option<&Hysteria2UdpUpstream> {
        self.upstreams.get(&(target.clone(), port))
    }

    /// Get or create an upstream for a target.
    pub async fn get_or_create_upstream(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        target: Address,
        port: u16,
        server: String,
        server_port: u16,
        password: String,
        initial_payload: Vec<u8>,
    ) -> Result<(), EngineError> {
        let key = (target.clone(), port);

        if let Some(upstream) = self.upstreams.get(&key) {
            let payload_len = initial_payload.len() as u64;
            proxy.record_session_inbound_rx(upstream.session_id, payload_len);
            let _ = upstream.send_tx.send(initial_payload).await;
            proxy.record_session_outbound_tx(upstream.session_id, payload_len);
            return Ok(());
        }

        match establish_hysteria2_udp_upstream(
            proxy,
            session,
            &server,
            server_port,
            &password,
            None, // client_fingerprint not stored in manager
            &initial_payload,
        )
        .await
        {
            Ok((upstream, mut recv_rx)) => {
                let session_id = upstream.session_id;
                self.upstreams.insert(key, upstream);

                self.response_tasks.spawn(async move {
                    let payload = recv_rx.recv().await.ok_or_else(|| {
                        EngineError::Io(std::io::Error::other("hysteria2 upstream channel closed"))
                    })?;
                    Ok((target, port, payload, Some(session_id)))
                });

                Ok(())
            }
            Err(error) => {
                error!(%error, "hysteria2 udp upstream establish failed");
                Err(error)
            }
        }
    }

    /// Poll for the next response from any upstream.
    pub async fn next_response(
        &mut self,
    ) -> Option<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>> {
        self.response_tasks.join_next().await.map(|res| match res {
            Ok(inner) => inner,
            Err(e) => Err(EngineError::Io(std::io::Error::other(format!(
                "hysteria2 upstream task failed: {e}"
            )))),
        })
    }
}
