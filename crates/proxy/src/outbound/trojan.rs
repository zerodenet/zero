//! Trojan UDP outbound — connect TLS, send CMD_UDP, relay datagrams.
//!
//! Trojan UDP tunnels SOCKS5-style UDP packets over a TCP/TLS stream.
//! Each packet is prefixed with a 2-byte big-endian length, followed
//! by the address, port, and payload.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::warn;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_protocol_trojan::{build_udp_packet, build_udp_request, read_udp_packet};

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

// ── Types ───────────────────────────────────────────────────────────

/// A decrypted response from a Trojan upstream.
#[derive(Debug, Clone)]
pub struct TrojanDecrypted {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

type TrojanUpstreamKey = (String, u16); // (server, port)

struct TrojanCachedUpstream {
    send_tx: mpsc::Sender<Vec<u8>>,
}

// ── Global state ────────────────────────────────────────────────────

static TROJAN_PENDING: LazyLock<Arc<Mutex<VecDeque<TrojanDecrypted>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(VecDeque::new())));

static TROJAN_CACHE: LazyLock<Mutex<HashMap<TrojanUpstreamKey, TrojanCachedUpstream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── Public API ──────────────────────────────────────────────────────

/// Drain all pending Trojan UDP responses (synchronous).
pub fn drain_all_trojan_responses() -> Vec<TrojanDecrypted> {
    TROJAN_PENDING
        .lock()
        .expect("trojan pending lock poisoned")
        .drain(..)
        .collect()
}

/// Send a UDP payload through a Trojan upstream, creating one if needed.
///
/// Called by the SOCKS5 UDP associate dispatch path.
pub async fn send_trojan_udp_packet(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    server_port: u16,
    password: &str,
    sni: Option<&str>,
    insecure: bool,
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let sent = payload.len();
    let key: TrojanUpstreamKey = (server.to_owned(), server_port);

    // Reuse existing upstream if available.
    let cached_tx = {
        let cache = TROJAN_CACHE.lock().expect("trojan cache lock poisoned");
        cache.get(&key).map(|u| u.send_tx.clone())
    };
    if let Some(tx) = cached_tx {
        let pkt = build_udp_packet(target, target_port, payload);
        let _ = tx.send(pkt).await;
        proxy.record_session_outbound_tx(session.id, sent as u64);
        return Ok(sent);
    }

    // Establish new upstream (TLS + CMD_UDP).
    let (send_tx, _bridge_target, _bridge_port) = establish_trojan_upstream(
        proxy, server, server_port, password, sni, insecure, session,
    )
    .await?;

    // Cache for reuse.
    TROJAN_CACHE
        .lock()
        .expect("trojan cache lock poisoned")
        .insert(
            key,
            TrojanCachedUpstream {
                send_tx: send_tx.clone(),
            },
        );

    // Send initial payload.
    let pkt = build_udp_packet(target, target_port, payload);
    let _ = send_tx.send(pkt).await;
    proxy.record_session_outbound_tx(session.id, sent as u64);
    Ok(sent)
}

// ── Internal ────────────────────────────────────────────────────────

async fn establish_trojan_upstream(
    proxy: &Proxy,
    server: &str,
    port: u16,
    password: &str,
    sni: Option<&str>,
    insecure: bool,
    session: &Session,
) -> Result<(mpsc::Sender<Vec<u8>>, Address, u16), EngineError> {
    use zero_config::ClientTlsConfig;

    // TCP connect
    let upstream = proxy
        .protocols
        .direct_outbound
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    // TLS
    let tls_config = ClientTlsConfig {
        server_name: sni.map(|s| s.to_owned()),
        disable_sni: false,
        ca_cert_path: None,
        insecure,
        alpn: Vec::new(),
    };
    let tls_stream = zero_transport::tls::connect_tls_upstream(
        upstream,
        &tls_config,
        proxy.config.source_dir(),
        server,
    )
    .await?;

    let mut metered = MeteredStream::new(TcpRelayStream::new(tls_stream));

    // Send CMD_UDP request
    let req = build_udp_request(password, &session.target, session.port)?;
    AsyncWriteExt::write_all(&mut metered, &req)
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    AsyncWriteExt::flush(&mut metered)
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;

    let traffic = metered.drain_traffic();
    proxy.record_session_outbound_traffic(session.id, traffic);

    let stream = Arc::new(tokio::sync::Mutex::new(metered.into_inner()));
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let bridge_target = session.target.clone();
    let bridge_port = session.port;

    // Send task: reads outgoing packets, writes framed data to stream.
    let send_stream = stream.clone();
    tokio::spawn(async move {
        while let Some(pkt) = send_rx.recv().await {
            let mut s = send_stream.lock().await;
            if let Err(e) = AsyncWriteExt::write_all(&mut *s, &pkt).await {
                warn!(error = %e, "trojan udp write failed");
                break;
            }
            if let Err(e) = AsyncWriteExt::flush(&mut *s).await {
                warn!(error = %e, "trojan udp flush failed");
                break;
            }
        }
    });

    // Recv task: reads framed data from stream, pushes to global pending queue.
    let recv_stream = stream.clone();
    let recv_target = bridge_target.clone();
    tokio::spawn(async move {
        loop {
            let mut s = recv_stream.lock().await;
            match read_udp_packet(&mut *s).await {
                Ok((addr, port, payload)) => {
                    drop(s);
                    let mut pending =
                        TROJAN_PENDING.lock().expect("trojan pending lock poisoned");
                    pending.push_back(TrojanDecrypted {
                        target: addr,
                        port,
                        payload,
                    });
                }
                Err(e) => {
                    warn!(error = %e, target = ?recv_target, "trojan udp read failed");
                    break;
                }
            }
        }
    });

    Ok((send_tx, bridge_target, bridge_port))
}
