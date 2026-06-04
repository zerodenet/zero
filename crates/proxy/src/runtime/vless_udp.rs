//! VLESS UDP types used by both inbound and outbound.
//!
//! Moved from outbound/vless.rs so inbound can import them without
//! depending on the outbound module.

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;
use zero_protocol_vless::parse_uuid;
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub(crate) struct VlessUdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

/// Transport options for VLESS UDP upstream connections.
#[derive(Clone, Copy)]
pub(crate) struct VlessUdpTransport<'a> {
    pub tls: Option<&'a ClientTlsConfig>,
    pub reality: Option<&'a RealityConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
    pub http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub split_http: Option<&'a SplitHttpConfig>,
    pub quic: Option<&'a QuicConfig>,
}

/// Spawn the bidirectional meter + relay task for a VLESS UDP upstream,
/// returning the upstream handle and a broadcast sender for responses.
fn spawn_vless_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    mut metered: MeteredStream<TcpRelayStream>,
    initial_payload_len: usize,
) -> (VlessUdpUpstream, broadcast::Sender<Vec<u8>>) {
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, _) = broadcast::channel::<Vec<u8>>(32);
    let recv_tx_bg = recv_tx.clone();

    proxy.record_session_outbound_tx(session_id, initial_payload_len as u64);

    let proxy_clone = proxy.clone();
    tokio::spawn(async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(payload) => {
                            if metered.write_all(&payload).await.is_err() {
                                break;
                            }
                            proxy_clone.record_session_outbound_tx(session_id, payload.len() as u64);
                        }
                        None => break,
                    }
                }
                read = metered.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            if recv_tx_bg.send(buffer[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    (
        VlessUdpUpstream {
            session_id,
            send_tx,
        },
        recv_tx,
    )
}

/// Establishes a VLESS UDP upstream connection with optional transport encryption.
pub(crate) async fn establish_vless_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    initial_payload: &[u8],
    transport: Option<&VlessUdpTransport<'_>>,
) -> Result<(VlessUdpUpstream, broadcast::Sender<Vec<u8>>), EngineError> {
    let vless_id = parse_uuid(id)?;

    // QUIC uses UDP — handle before TCP connect entirely
    if let Some(t) = transport {
        if let Some(quic) = t.quic {
            let server_name = quic.server_name.as_deref().unwrap_or(server);
            let quic_stream =
                crate::transport::connect_quic(server_name, port, quic.insecure).await?;

            let mut metered = MeteredStream::new(TcpRelayStream::new(quic_stream));
            proxy
                .protocols
                .vless_outbound
                .send_udp_request(&mut metered, session, &vless_id)
                .await?;
            metered.write_all(initial_payload).await?;

            return Ok(spawn_vless_udp_relay(
                proxy,
                session.id,
                metered,
                initial_payload.len(),
            ));
        }
    }

    let socket = proxy
        .protocols
        .direct_outbound
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream: TcpRelayStream = match transport {
        Some(t) => {
            let connector = crate::transport::VlessTransportConnector::new(
                t.tls,
                t.reality,
                t.ws,
                t.grpc,
                t.h2,
                t.http_upgrade,
                t.split_http,
                proxy.config.source_dir(),
            );
            connector.connect(socket, server, port).await?
        }
        None => socket.into(),
    };

    let mut metered = MeteredStream::new(stream);

    proxy
        .protocols
        .vless_outbound
        .send_udp_request(&mut metered, session, &vless_id)
        .await?;
    metered.write_all(initial_payload).await?;

    Ok(spawn_vless_udp_relay(
        proxy,
        session.id,
        metered,
        initial_payload.len(),
    ))
}

/// VLESS UDP outbound manager — manages per-target upstream connections.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(crate) struct VlessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), (VlessUdpUpstream, broadcast::Sender<Vec<u8>>)>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    /// Check if an upstream already exists for a target.
    pub fn get(&self, target: &Address, port: u16) -> Option<&VlessUdpUpstream> {
        self.upstreams
            .get(&(target.clone(), port))
            .map(|(upstream, _)| upstream)
    }

    /// Spawn a one-shot bridge task for a cached upstream.
    pub(crate) fn spawn_bridge(
        &self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        target: Address,
        port: u16,
        session_id: u64,
    ) {
        if let Some((_, recv_tx)) = self.upstreams.get(&(target.clone(), port)) {
            let mut recv_rx = recv_tx.subscribe();
            let t = target.clone();
            chain_tasks.spawn(async move {
                let payload = recv_rx.recv().await.map_err(|_| {
                    EngineError::Io(std::io::Error::other("vless upstream closed"))
                })?;
                Ok((t, port, payload, Some(session_id)))
            });
        }
    }

    /// Get or create an upstream for a target.
    /// Spawns a bridge task into `chain_tasks` for response polling.
    pub async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        proxy: &Proxy,
        session: &Session,
        target: Address,
        port: u16,
        server: String,
        server_port: u16,
        id: String,
        initial_payload: Vec<u8>,
        transport: Option<&VlessUdpTransport<'_>>,
    ) -> Result<(), EngineError> {
        let key = (target.clone(), port);

        if let Some((upstream, _)) = self.upstreams.get(&key) {
            proxy.record_session_inbound_rx(upstream.session_id, initial_payload.len() as u64);
            let payload_len = initial_payload.len() as u64;
            let _ = upstream.send_tx.send(initial_payload).await;
            proxy.record_session_outbound_tx(upstream.session_id, payload_len);
            // Spawn bridge for the expected response
            self.spawn_bridge(chain_tasks, target, port, upstream.session_id);
            return Ok(());
        }

        match establish_vless_udp_upstream(
            proxy,
            session,
            &server,
            server_port,
            &id,
            &initial_payload,
            transport,
        )
        .await
        {
            Ok((upstream, recv_tx)) => {
                let session_id = upstream.session_id;
                self.upstreams.insert(key, (upstream, recv_tx));
                // Spawn bridge for the first response
                self.spawn_bridge(chain_tasks, target, port, session_id);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
