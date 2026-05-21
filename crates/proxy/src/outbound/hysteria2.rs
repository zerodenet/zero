//! Hysteria2 outbound protocol implementation.
//!
//! Provides UDP upstream establishment and management for chained
//! Hysteria2 connections, following the same pattern as VLESS.
//!
//! TODO: wire UDP relay (establish_hysteria2_udp_upstream, etc.)
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

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
    initial_payload: &[u8],
) -> Result<(Hysteria2UdpUpstream, mpsc::Receiver<Vec<u8>>), EngineError> {
    let connector = Hysteria2Connector::new(server, port, password);
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
pub struct Hysteria2UdpOutboundManager {
    upstreams: HashMap<(Address, u16), Hysteria2UdpUpstream>,
    response_tasks: JoinSet<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>>,
}

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
