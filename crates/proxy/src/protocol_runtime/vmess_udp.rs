//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

pub(crate) mod model;

use std::collections::HashMap;

use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use model::{VmessUdpRelayFlowStart, VmessUdpStartFlow, VmessUdpUpstream, VmessUdpUpstreamRequest};

type VmessResponseSender = broadcast::Sender<vmess::VmessUdpFlowResponse>;

fn upstream_from_stream(
    session_id: u64,
    flow: vmess::VmessUdpFlowHandle,
) -> (VmessUdpUpstream, VmessResponseSender) {
    (
        VmessUdpUpstream {
            session_id,
            sender: flow.sender,
        },
        flow.responses,
    )
}

async fn establish_vmess_udp_upstream_over_stream(
    proxy: &Proxy,
    session: &Session,
    identity: vmess::VmessUdpIdentity,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<(VmessUdpUpstream, VmessResponseSender), EngineError> {
    let (flow, initial_packet_len) =
        vmess::open_udp_flow(stream, session, identity, initial_payload).await?;
    proxy.record_session_outbound_tx(session.id, initial_packet_len as u64);
    Ok(upstream_from_stream(session.id, flow))
}

async fn establish_vmess_udp_upstream(
    request: &VmessUdpUpstreamRequest<'_>,
) -> Result<(VmessUdpUpstream, VmessResponseSender), EngineError> {
    let initial_packet = vmess::encode_udp_flow_initial_packet(
        &request.session.target,
        request.session.port,
        request.initial_payload,
    )?;

    if let Some(max_concurrency) = request.mux_concurrency {
        let mux_stream = request
            .proxy
            .vmess_mux_pool
            .open_udp_stream(
                crate::protocol_runtime::vmess_mux_pool::model::VmessMuxOpenRequest {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server.to_owned(),
                    port: request.server_port,
                    id: request.identity.uuid,
                    cipher_name: request.cipher_name.to_owned(),
                    cipher: request.identity.cipher,
                    tls: request.transport.and_then(|transport| transport.tls),
                    ws: request.transport.and_then(|transport| transport.ws),
                    grpc: request.transport.and_then(|transport| transport.grpc),
                    max_concurrency,
                },
            )
            .await?;
        let initial_packet_len = initial_packet.len();
        let flow = vmess::open_mux_udp_flow(mux_stream, initial_packet);
        request
            .proxy
            .record_session_outbound_tx(request.session.id, initial_packet_len as u64);
        return Ok(upstream_from_stream(request.session.id, flow));
    }

    let socket = request
        .proxy
        .protocols
        .direct_connector()
        .connect_host(
            request.server,
            request.server_port,
            request.proxy.resolver.as_ref(),
        )
        .await?;

    let stream = match request.transport {
        Some(transport) => {
            let connector = crate::transport::VmessTransportConnector::new(*transport);
            connector
                .connect(socket, request.server, request.server_port)
                .await?
        }
        None => socket.into(),
    };

    establish_vmess_udp_upstream_over_stream(
        request.proxy,
        request.session,
        request.identity,
        request.initial_payload,
        stream,
    )
    .await
}

pub(crate) struct VmessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), (VmessUdpUpstream, VmessResponseSender)>,
}

impl VmessUdpOutboundManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.get_or_create_upstream(
            chain_tasks,
            VmessUdpUpstreamRequest {
                proxy: request.proxy,
                session: request.session,
                target: request.session.target.clone(),
                port: request.session.port,
                server: request.server,
                server_port: request.port,
                identity: request.identity,
                cipher_name: request.cipher_name,
                initial_payload: request.payload,
                transport: Some(&request.transport),
                mux_concurrency: request.mux_concurrency,
            },
        )
        .await
    }

    pub(crate) async fn start_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vmess_outbound_transport_over_stream(
            crate::transport::VmessFinalHopTransportRequest {
                carrier: request.carrier,
                options: request.transport,
            },
        )
        .await?;
        let (upstream, recv_tx) = establish_vmess_udp_upstream_over_stream(
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
        let packet_len = upstream.sender.send(target, port, payload).await? as u64;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VmessUdpUpstream,
        recv_tx: VmessResponseSender,
    ) {
        self.upstreams.insert(key, (upstream, recv_tx));
    }

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
                    .map_err(|_| EngineError::Io(std::io::Error::other("vmess upstream closed")))?;
                Ok((packet.0, packet.1, packet.2, Some(session_id)))
            });
        }
    }

    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::protocol_runtime::udp::ChainTask>,
        request: VmessUdpUpstreamRequest<'_>,
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
            self.spawn_bridge(
                chain_tasks,
                request.target,
                request.port,
                upstream.session_id,
            );
            return Ok(());
        }

        let (upstream, recv_tx) = establish_vmess_udp_upstream(&request).await?;
        let session_id = upstream.session_id;
        self.upstreams.insert(key, (upstream, recv_tx));
        self.spawn_bridge(chain_tasks, request.target, request.port, session_id);
        Ok(())
    }
}
