//! VLESS UDP types used by both inbound and outbound.
//!
//! Moved from outbound/vless.rs so inbound can import them without
//! depending on the outbound module.

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use vless::parse_uuid;
use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;
use zero_traits::{AsyncSocket, UdpPacketFraming, UdpPacketTunnelProtocol};

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub(super) struct VlessUdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

/// Transport options for VLESS UDP upstream connections.
#[derive(Clone, Copy)]
pub(crate) struct VlessUdpTransport<'a> {
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) reality: Option<&'a RealityConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
    pub(crate) h2: Option<&'a H2Config>,
    pub(crate) http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a SplitHttpConfig>,
    pub(crate) quic: Option<&'a QuicConfig>,
}

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a str,
    pub(crate) flow: Option<&'a str>,
    pub(crate) transport: VlessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) split_http: &'a SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHop<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) transport: VlessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(super) struct VlessUdpUpstreamRequest<'a> {
    proxy: &'a Proxy,
    session: &'a Session,
    target: Address,
    port: u16,
    server: &'a str,
    server_port: u16,
    id: &'a str,
    initial_payload: &'a [u8],
    transport: Option<&'a VlessUdpTransport<'a>>,
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
    let vless_outbound = proxy.protocols.vless_outbound_protocol();

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
                            match <vless::VlessOutbound as UdpPacketFraming<
                                vless::VlessUdpPacketTarget,
                            >>::decode_udp_packet(&vless_outbound, &buffer[..n]) {
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
    id: &str,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<(VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>), EngineError> {
    let vless_id = parse_uuid(id)?;
    let initial_packet =
        <vless::VlessOutbound as UdpPacketFraming<vless::VlessUdpPacketTarget>>::encode_udp_packet(
            &proxy.protocols.vless_outbound_protocol(),
            &vless::VlessUdpPacketTarget {
                address: &session.target,
                port: session.port,
                payload: initial_payload,
            },
        )?;

    let mut metered = MeteredStream::new(stream);

    <vless::VlessOutbound as UdpPacketTunnelProtocol<vless::VlessUdpPacketTunnelTarget>>::establish_udp_packet_tunnel(
        &proxy.protocols.vless_outbound_protocol(),
        &mut metered,
        &vless::VlessUdpPacketTunnelTarget {
            session,
            id: &vless_id,
        },
    )
    .await?;
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
    id: &str,
    initial_payload: &[u8],
    transport: Option<&VlessUdpTransport<'_>>,
) -> Result<(VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>), EngineError> {
    let vless_id = parse_uuid(id)?;
    let initial_packet =
        <vless::VlessOutbound as UdpPacketFraming<vless::VlessUdpPacketTarget>>::encode_udp_packet(
            &proxy.protocols.vless_outbound_protocol(),
            &vless::VlessUdpPacketTarget {
                address: &session.target,
                port: session.port,
                payload: initial_payload,
            },
        )?;

    // QUIC uses UDP — handle before TCP connect entirely
    if let Some(t) = transport {
        if let Some(quic) = t.quic {
            let server_name = quic.server_name.as_deref().unwrap_or(server);
            let quic_stream =
                crate::transport::connect_quic(server_name, port, quic.insecure).await?;

            let mut metered = MeteredStream::new(TcpRelayStream::new(quic_stream));
            <vless::VlessOutbound as UdpPacketTunnelProtocol<
                vless::VlessUdpPacketTunnelTarget,
            >>::establish_udp_packet_tunnel(
                &proxy.protocols.vless_outbound_protocol(),
                &mut metered,
                &vless::VlessUdpPacketTunnelTarget {
                    session,
                    id: &vless_id,
                },
            )
            .await?;
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

    establish_vless_udp_upstream_over_stream(proxy, session, id, initial_payload, stream).await
}

/// VLESS UDP outbound manager — manages per-target upstream connections.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(super) struct VlessUdpOutboundManager {
    upstreams:
        HashMap<(Address, u16), (VlessUdpUpstream, broadcast::Sender<vless::VlessUdpPacket>)>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(super) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        request: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        use zero_traits::UdpPacketFraming;

        let mux_flow_enabled = request.flow == Some("xtls-rprx-vision")
            || request.flow == Some("xtls-rprx-vision-udp443");
        if mux_flow_enabled {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = request
                .proxy
                .mux_pool
                .open_udp_stream(crate::runtime::mux_pool::VlessMuxOpenRequest {
                    proxy: request.proxy,
                    session: None,
                    server: request.server,
                    port: request.port,
                    id: &::vless::parse_uuid(request.id).map_err(|error| {
                        EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("invalid VLESS UUID: {error}"),
                        ))
                    })?,
                    tls: request.transport.tls,
                    reality: request.transport.reality,
                    max_concurrency,
                })
                .await
            {
                let packet = <::vless::VlessOutbound as UdpPacketFraming<
                    ::vless::VlessUdpPacketTarget,
                >>::encode_udp_packet(
                    &request.proxy.protocols.vless_outbound_protocol(),
                    &::vless::VlessUdpPacketTarget {
                        address: &request.session.target,
                        port: request.session.port,
                        payload: request.payload,
                    },
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
                id: request.id,
                initial_payload: request.payload,
                transport: Some(&request.transport),
            },
        )
        .await
    }

    pub(super) async fn start_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
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
            request.id,
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

    pub(super) async fn start_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
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
            request.id,
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
    pub(super) async fn send_existing(
        &self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let Some((upstream, _)) = self.upstreams.get(&(target.clone(), port)) else {
            return Ok(None);
        };

        proxy.record_session_inbound_rx(upstream.session_id, payload.len() as u64);
        let packet = <vless::VlessOutbound as UdpPacketFraming<
            vless::VlessUdpPacketTarget,
        >>::encode_udp_packet(
            &proxy.protocols.vless_outbound_protocol(),
            &vless::VlessUdpPacketTarget {
                address: target,
                port,
                payload,
            },
        )?;
        let packet_len = packet.len() as u64;
        let _ = upstream.send_tx.send(packet).await;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    pub(super) fn insert_upstream(
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
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
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
    pub(super) async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        request: VlessUdpUpstreamRequest<'_>,
    ) -> Result<(), EngineError> {
        let key = (request.target.clone(), request.port);

        if let Some((upstream, _)) = self.upstreams.get(&key) {
            request.proxy.record_session_inbound_rx(
                upstream.session_id,
                request.initial_payload.len() as u64,
            );
            let packet = <vless::VlessOutbound as UdpPacketFraming<
                vless::VlessUdpPacketTarget,
            >>::encode_udp_packet(
                &request.proxy.protocols.vless_outbound_protocol(),
                &vless::VlessUdpPacketTarget {
                    address: &request.target,
                    port: request.port,
                    payload: request.initial_payload,
                },
            )?;
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
            request.id,
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
