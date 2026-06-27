//! VLESS UDP outbound manager.
//!
//! Protocol packet framing stays in `protocols/vless`; this module owns proxy
//! transport opening, cached upstream streams, metering, and response bridges.

pub(crate) mod model;

use std::collections::HashMap;

use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_core::{Address, Session, UdpFlowPacket};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::runtime::udp_flow::managed::ManagedCachedFlowSender;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow, VlessUdpUpstream,
    VlessUdpUpstreamRequest,
};

type VlessFlowResponse = (Address, u16, Vec<u8>);
type VlessResponseSender = broadcast::Sender<VlessFlowResponse>;

fn upstream_from_stream(
    session_id: u64,
    flow: vless::VlessUdpFlowHandle,
) -> (VlessUdpUpstream, VlessResponseSender) {
    (
        VlessUdpUpstream {
            session_id,
            sender: flow.sender,
        },
        flow.responses,
    )
}

/// Establishes a VLESS UDP upstream over an already connected stream.
async fn establish_vless_udp_upstream_over_stream(
    proxy: &Proxy,
    session: &Session,
    identity: vless::VlessUdpIdentity,
    initial_payload: &[u8],
    mut stream: TcpRelayStream,
) -> Result<(VlessUdpUpstream, VlessResponseSender), EngineError> {
    let flow_io = vless::establish_udp_flow(&mut stream, session, identity).await?;
    let initial_packet = UdpFlowPacket::from_parts(&session.target, session.port, initial_payload);
    let initial_packet_len = flow_io
        .initial_packet(
            &initial_packet.target,
            initial_packet.port,
            &initial_packet.payload,
        )
        .map_err(EngineError::from)?
        .len();
    let flow = vless::spawn_udp_flow(stream, Some(initial_packet), flow_io);
    proxy.record_session_outbound_tx(session.id, initial_packet_len as u64);
    Ok(upstream_from_stream(session.id, flow))
}

/// Establishes a VLESS UDP upstream connection with optional transport encryption.
async fn establish_vless_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    identity: vless::VlessUdpIdentity,
    initial_payload: &[u8],
    transport: Option<&crate::transport::VlessUdpTransportOptions<'_>>,
) -> Result<(VlessUdpUpstream, VlessResponseSender), EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream: TcpRelayStream = match transport {
        Some(t) => {
            let connector = crate::transport::VlessUdpTransportConnector::new(*t);
            connector.connect(socket, server, port).await?
        }
        None => socket.into(),
    };

    establish_vless_udp_upstream_over_stream(proxy, session, identity, initial_payload, stream)
        .await
}

/// VLESS UDP outbound manager -?manages per-target upstream connections.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(crate) struct VlessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), (VlessUdpUpstream, VlessResponseSender)>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        let mux_flow_enabled = request.flow == Some("xtls-rprx-vision")
            || request.flow == Some("xtls-rprx-vision-udp443");
        if mux_flow_enabled {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = request
                .mux_pool
                .open_udp_stream(VlessMuxOpenRequest {
                    proxy: request.proxy,
                    session: None,
                    server: request.server,
                    port: request.port,
                    id: &request.identity.uuid,
                    tls: request.transport.tls,
                    reality: request.transport.reality,
                    max_concurrency,
                })
                .await
            {
                let initial_packet = UdpFlowPacket::from_parts(
                    &request.session.target,
                    request.session.port,
                    request.payload,
                );
                let flow_io = vless::VlessEstablishedUdpFlow::default();
                let packet = flow_io.initial_packet(
                    &initial_packet.target,
                    initial_packet.port,
                    &initial_packet.payload,
                )?;
                let sent = packet.len();
                let _ = up_tx.send(packet);
                request
                    .proxy
                    .record_session_outbound_tx(request.session.id, sent as u64);
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
                identity: request.identity,
                initial_payload: request.payload,
                transport: Some(&request.transport),
            },
        )
        .await
    }

    pub(crate) async fn start_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
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
            request.identity,
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
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpRelayFinalHopStart<'_>,
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
                    source_dir: request.transport.source_dir,
                },
            },
        )
        .await?;
        let (upstream, recv_tx) = establish_vless_udp_upstream_over_stream(
            request.proxy,
            request.session,
            request.identity,
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
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let Some((upstream, _)) = self.upstreams.get(&(target.clone(), port)) else {
            return Ok(None);
        };

        proxy.record_session_inbound_rx(upstream.session_id, payload.len() as u64);
        let packet_len = upstream.sender.send(target, port, payload).await? as u64;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VlessUdpUpstream,
        recv_tx: VlessResponseSender,
    ) {
        self.upstreams.insert(key, (upstream, recv_tx));
    }

    /// Spawn a one-shot bridge task for a cached upstream.
    pub(super) fn spawn_bridge(
        &self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
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
                Ok((packet.0, packet.1, packet.2, Some(session_id)))
            });
        }
    }

    /// Get or create an upstream for a target.
    /// Spawns a bridge task into `chain_tasks` for response polling.
    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpUpstreamRequest<'_>,
    ) -> Result<(), EngineError> {
        let key = (request.target.clone(), request.port);

        if let Some((upstream, _)) = self.upstreams.get(&key) {
            request.proxy.record_session_inbound_rx(
                upstream.session_id,
                request.initial_payload.len() as u64,
            );
            let packet_len = upstream
                .sender
                .send(&request.target, request.port, request.initial_payload)
                .await? as u64;
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
            request.identity,
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

#[async_trait::async_trait]
impl ManagedCachedFlowSender for VlessUdpOutboundManager {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        VlessUdpOutboundManager::send_existing(self, chain_tasks, proxy, target, port, payload)
            .await
    }
}
