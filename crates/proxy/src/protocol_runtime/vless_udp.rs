//! VLESS UDP types used by both inbound and outbound.
//!
//! Moved from outbound/vless.rs so inbound can import them without
//! depending on the outbound module.

pub(super) mod model;

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
use model::{
    VlessUdpRelayFinalHop, VlessUdpRelayTwoStream, VlessUdpStartFlow, VlessUdpTransport,
    VlessUdpUpstream, VlessUdpUpstreamRequest,
};

fn encode_vless_udp_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    vless::encode_udp_flow_packet(target, port, payload).map_err(EngineError::from)
}

fn decode_vless_udp_packet(packet: &[u8]) -> Result<vless::VlessUdpPacket, EngineError> {
    vless::decode_udp_flow_packet(packet).map_err(EngineError::from)
}

/// Spawn the bidirectional meter + relay task for a VLESS UDP upstream,
/// returning the upstream handle and a broadcast sender for decoded responses.
fn spawn_vless_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    mut metered: MeteredStream<TcpRelayStream>,
    initial_payload_len: usize,
) -> (VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>) {
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, _) = broadcast::channel::<vless::VlessUdpPacket>(32);
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
                            match decode_vless_udp_packet(&buffer[..n]) {
                                Ok(packet) => {
                                    if recv_tx_bg.send(packet).is_err() {
                                        break;
                                    }
                                }
                                Err(error) => {
                                    tracing::debug!(error = %error, "failed to decode VLESS UDP packet");
                                    break;
                                }
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

/// Establishes a VLESS UDP upstream over an already connected stream.
async fn establish_vless_udp_upstream_over_stream(
    proxy: &Proxy,
    session: &Session,
    uuid: [u8; 16],
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<(VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>), EngineError> {
    let initial_packet = encode_vless_udp_packet(&session.target, session.port, initial_payload)?;

    let mut metered = MeteredStream::new(stream);

    vless::establish_udp_packet_tunnel(&mut metered, session, &uuid).await?;
    metered.write_all(&initial_packet).await?;

    Ok(spawn_vless_udp_relay(
        proxy,
        session.id,
        metered,
        initial_packet.len(),
    ))
}

/// Establishes a VLESS UDP upstream connection with optional transport encryption.
async fn establish_vless_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    uuid: [u8; 16],
    initial_payload: &[u8],
    transport: Option<&VlessUdpTransport<'_>>,
) -> Result<(VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>), EngineError> {
    let initial_packet = encode_vless_udp_packet(&session.target, session.port, initial_payload)?;

    // QUIC uses UDP -?handle before TCP connect entirely
    if let Some(t) = transport {
        if let Some(quic) = t.quic {
            let server_name = quic.server_name.as_deref().unwrap_or(server);
            let quic_stream =
                crate::transport::connect_quic(server_name, port, quic.insecure).await?;

            let mut metered = MeteredStream::new(TcpRelayStream::new(quic_stream));
            vless::establish_udp_packet_tunnel(&mut metered, session, &uuid).await?;
            metered.write_all(&initial_packet).await?;

            return Ok(spawn_vless_udp_relay(
                proxy,
                session.id,
                metered,
                initial_packet.len(),
            ));
        }
    }

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream: TcpRelayStream = match transport {
        Some(t) => {
            let connector = crate::transport::VlessTransportConnector::new(
                crate::transport::VlessTransportOptions {
                    tls: t.tls,
                    reality: t.reality,
                    ws: t.ws,
                    grpc: t.grpc,
                    h2: t.h2,
                    http_upgrade: t.http_upgrade,
                    split_http: t.split_http,
                    source_dir: proxy.config.source_dir(),
                },
            );
            connector.connect(socket, server, port).await?
        }
        None => socket.into(),
    };

    establish_vless_udp_upstream_over_stream(proxy, session, uuid, initial_payload, stream).await
}

/// VLESS UDP outbound manager -?manages per-target upstream connections.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(crate) struct VlessUdpOutboundManager {
    upstreams:
        HashMap<(Address, u16), (VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>)>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        let mux_flow_enabled = request.flow == Some("xtls-rprx-vision")
            || request.flow == Some("xtls-rprx-vision-udp443");
        if mux_flow_enabled {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = request
                .proxy
                .mux_pool
                .open_udp_stream(
                    crate::protocol_runtime::vless_mux_pool::model::VlessMuxOpenRequest {
                        proxy: request.proxy,
                        session: None,
                        server: request.server,
                        port: request.port,
                        id: &request.uuid,
                        tls: request.transport.tls,
                        reality: request.transport.reality,
                        max_concurrency,
                    },
                )
                .await
            {
                let packet = encode_vless_udp_packet(
                    &request.session.target,
                    request.session.port,
                    request.payload,
                )?;
                let _ = up_tx.send(packet);
                request
                    .proxy
                    .record_session_outbound_tx(request.session.id, request.payload.len() as u64);
                return Ok(());
            }
        }

        self.get_or_create_upstream(
            chain_tasks,
            VlessUdpUpstreamRequest {
                proxy: request.proxy,
                session: request.session,
                target: request.session.target.clone(),
                port: request.session.port,
                server: request.server,
                server_port: request.port,
                uuid: request.uuid,
                initial_payload: request.payload,
                transport: Some(&request.transport),
            },
        )
        .await
    }

    pub(crate) async fn start_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vless_split_http_over_relay(
            request.post_carrier.stream,
            request.get_carrier.stream,
            request.split_http,
        )
        .await?;
        let (upstream, recv_tx) = establish_vless_udp_upstream_over_stream(
            request.proxy,
            request.session,
            request.uuid,
            request.payload,
            stream,
        )
        .await?;
        self.insert_upstream(
            (request.session.target.clone(), request.session.port),
            upstream,
            recv_tx,
        );
        self.spawn_bridge(
            chain_tasks,
            request.session.target.clone(),
            request.session.port,
            request.session.id,
        );
        Ok(())
    }

    pub(crate) async fn start_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VlessUdpRelayFinalHop<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vless_outbound_transport_over_stream(
            crate::transport::VlessFinalHopTransportRequest {
                carrier: request.carrier,
                options: crate::transport::VlessTransportOptions {
                    tls: request.transport.tls,
                    reality: request.transport.reality,
                    ws: request.transport.ws,
                    grpc: request.transport.grpc,
                    h2: request.transport.h2,
                    http_upgrade: request.transport.http_upgrade,
                    split_http: request.transport.split_http,
                    source_dir: request.proxy.config.source_dir(),
                },
            },
        )
        .await?;
        let (upstream, recv_tx) = establish_vless_udp_upstream_over_stream(
            request.proxy,
            request.session,
            request.uuid,
            request.payload,
            stream,
        )
        .await?;
        self.insert_upstream(
            (request.session.target.clone(), request.session.port),
            upstream,
            recv_tx,
        );
        self.spawn_bridge(
            chain_tasks,
            request.session.target.clone(),
            request.session.port,
            request.session.id,
        );
        Ok(())
    }

    /// Send a packet through an existing upstream, if one is cached.
    pub(crate) async fn send_existing(
        &self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let Some((upstream, _)) = self.upstreams.get(&(target.clone(), port)) else {
            return Ok(None);
        };

        proxy.record_session_inbound_rx(upstream.session_id, payload.len() as u64);
        let packet = encode_vless_udp_packet(target, port, payload)?;
        let packet_len = packet.len() as u64;
        let _ = upstream.send_tx.send(packet).await;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VlessUdpUpstream,
        recv_tx: broadcast::Sender<vless::VlessUdpPacket>,
    ) {
        self.upstreams.insert(key, (upstream, recv_tx));
    }

    /// Spawn a one-shot bridge task for a cached upstream.
    pub(super) fn spawn_bridge(
        &self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        target: Address,
        port: u16,
        session_id: u64,
    ) {
        if let Some((_, recv_tx)) = self.upstreams.get(&(target.clone(), port)) {
            let mut recv_rx = recv_tx.subscribe();
            chain_tasks.spawn(async move {
                let packet = recv_rx
                    .recv()
                    .await
                    .map_err(|_| EngineError::Io(std::io::Error::other("vless upstream closed")))?;
                Ok((packet.target, packet.port, packet.payload, Some(session_id)))
            });
        }
    }

    /// Get or create an upstream for a target.
    /// Spawns a bridge task into `chain_tasks` for response polling.
    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VlessUdpUpstreamRequest<'_>,
    ) -> Result<(), EngineError> {
        let key = (request.target.clone(), request.port);

        if let Some((upstream, _)) = self.upstreams.get(&key) {
            request.proxy.record_session_inbound_rx(
                upstream.session_id,
                request.initial_payload.len() as u64,
            );
            let packet =
                encode_vless_udp_packet(&request.target, request.port, request.initial_payload)?;
            let packet_len = packet.len() as u64;
            let _ = upstream.send_tx.send(packet).await;
            request
                .proxy
                .record_session_outbound_tx(upstream.session_id, packet_len);
            // Spawn bridge for the expected response
            self.spawn_bridge(
                chain_tasks,
                request.target,
                request.port,
                upstream.session_id,
            );
            return Ok(());
        }

        match establish_vless_udp_upstream(
            request.proxy,
            request.session,
            request.server,
            request.server_port,
            request.uuid,
            request.initial_payload,
            request.transport,
        )
        .await
        {
            Ok((upstream, recv_tx)) => {
                let session_id = upstream.session_id;
                self.upstreams.insert(key, (upstream, recv_tx));
                // Spawn bridge for the first response
                self.spawn_bridge(chain_tasks, request.target, request.port, session_id);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
