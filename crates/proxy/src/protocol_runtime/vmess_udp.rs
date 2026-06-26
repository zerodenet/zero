//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

pub(crate) mod model;

use std::collections::HashMap;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session, UdpFlowPacket};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use model::{VmessUdpRelayFlowStart, VmessUdpStartFlow, VmessUdpUpstream, VmessUdpUpstreamRequest};

type VmessFlowResponse = (Address, u16, Vec<u8>);
type VmessResponseSender = broadcast::Sender<VmessFlowResponse>;

#[derive(Clone)]
pub(super) struct VmessFlowSender {
    send_tx: mpsc::Sender<UdpFlowPacket>,
}

impl VmessFlowSender {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let packet = UdpFlowPacket::from_parts(target, port, payload);
        let packet_len = vmess::VmessUdpFlowIo
            .encode_packet(target, port, payload)?
            .len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other("vmess udp flow closed")))?;
        Ok(packet_len)
    }
}

struct VmessFlowHandle {
    sender: VmessFlowSender,
    responses: VmessResponseSender,
}

fn upstream_from_stream(
    session_id: u64,
    flow: VmessFlowHandle,
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
    let stream = vmess::establish_udp_flow_stream(stream, session, identity).await?;
    let mut stream = stream;
    let initial_packet =
        vmess::encode_udp_flow_initial_packet(&session.target, session.port, initial_payload)?;
    let initial_packet_len = initial_packet.len();
    stream
        .write_all(&initial_packet)
        .await
        .map_err(|_| EngineError::Core(zero_core::Error::Io("vmess udp flow write")))?;
    stream
        .flush()
        .await
        .map_err(|_| EngineError::Core(zero_core::Error::Io("vmess udp flow flush")))?;
    let flow = spawn_udp_flow(stream, Vec::new());
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
        let flow = spawn_udp_flow(mux_stream, initial_packet);
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
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
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
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
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
        upstream: VmessUdpUpstream,
        recv_tx: VmessResponseSender,
    ) {
        self.upstreams.insert(key, (upstream, recv_tx));
    }

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
                    .map_err(|_| EngineError::Io(std::io::Error::other("vmess upstream closed")))?;
                Ok((packet.0, packet.1, packet.2, Some(session_id)))
            });
        }
    }

    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
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

fn spawn_udp_flow<S>(stream: S, initial_packet: Vec<u8>) -> VmessFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (responses, _) = broadcast::channel::<VmessFlowResponse>(32);
    spawn_udp_flow_task(stream, initial_packet, send_rx, responses.clone());
    VmessFlowHandle {
        sender: VmessFlowSender { send_tx },
        responses,
    }
}

fn spawn_udp_flow_task<S>(
    mut stream: S,
    initial_packet: Vec<u8>,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    responses: VmessResponseSender,
) where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    tokio::spawn(async move {
        if !initial_packet.is_empty() {
            if stream.write_all(&initial_packet).await.is_err() {
                return;
            }
            if stream.flush().await.is_err() {
                return;
            }
        }

        let flow_io = vmess::VmessUdpFlowIo;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(packet) => {
                            let encoded = match flow_io.encode_packet(&packet.target, packet.port, &packet.payload) {
                                Ok(encoded) => encoded,
                                Err(_) => break,
                            };
                            if stream.write_all(&encoded).await.is_err() {
                                break;
                            }
                            if stream.flush().await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = stream.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(packet) = flow_io.decode_packet(&buffer[..n]) {
                                let _ = responses.send(packet.into_parts());
                            } else {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });
}
