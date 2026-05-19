//! VLESS outbound protocol implementation

use std::collections::HashMap;

use tokio::sync::mpsc;
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
pub struct VlessUdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

/// Transport options for VLESS UDP upstream connections.
#[derive(Clone, Copy)]
pub struct VlessUdpTransport<'a> {
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
/// returning the upstream handle and receive channel.
fn spawn_vless_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    mut metered: MeteredStream<TcpRelayStream>,
    initial_payload_len: usize,
) -> (VlessUdpUpstream, mpsc::Receiver<Vec<u8>>) {
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, recv_rx) = mpsc::channel::<Vec<u8>>(32);

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
                            if recv_tx.send(buffer[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    (VlessUdpUpstream { session_id, send_tx }, recv_rx)
}

/// Establishes a VLESS UDP upstream connection with optional transport encryption.
pub async fn establish_vless_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    initial_payload: &[u8],
    transport: Option<&VlessUdpTransport<'_>>,
) -> Result<(VlessUdpUpstream, mpsc::Receiver<Vec<u8>>), EngineError> {
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
        .connect_host(server, port, &proxy.resolver)
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
pub struct VlessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), VlessUdpUpstream>,
    response_tasks: JoinSet<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
            response_tasks: JoinSet::new(),
        }
    }

    /// Check if an upstream already exists for a target.
    pub fn get(&self, target: &Address, port: u16) -> Option<&VlessUdpUpstream> {
        self.upstreams.get(&(target.clone(), port))
    }

    /// Get or create an upstream for a target
    pub async fn get_or_create_upstream(
        &mut self,
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

        if let Some(upstream) = self.upstreams.get(&key) {
            proxy.record_session_inbound_rx(upstream.session_id, initial_payload.len() as u64);
            let payload_len = initial_payload.len() as u64;
            let _ = upstream.send_tx.send(initial_payload).await;
            proxy.record_session_outbound_tx(upstream.session_id, payload_len);
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
        ).await {
            Ok((upstream, mut recv_rx)) => {
                let session_id = upstream.session_id;
                self.upstreams.insert(key, upstream);

                // Spawn response reader task
                self.response_tasks.spawn(async move {
                    let payload = recv_rx.recv().await
                        .ok_or_else(|| EngineError::Io(std::io::Error::other("upstream channel closed")))?;

                    Ok((target, port, payload, Some(session_id)))
                });

                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Poll for next response
    pub async fn next_response(&mut self) -> Option<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>> {
        self.response_tasks.join_next().await.map(|res| match res {
            Ok(inner) => inner,
            Err(e) => Err(EngineError::Io(std::io::Error::other(format!("upstream task failed: {}", e)))),
        })
    }
}
