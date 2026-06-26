//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

pub(super) mod model;

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;
use zero_traits::AsyncSocket;

use crate::protocol_runtime::udp::packet_path_traits::UdpResponsePacket;
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
use model::{VmessUdpRelayFlow, VmessUdpStartFlow, VmessUdpUpstream, VmessUdpUpstreamRequest};

fn spawn_vmess_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    mut metered: MeteredStream<TcpRelayStream>,
    initial_payload_len: usize,
) -> (VmessUdpUpstream, broadcast::Sender<UdpResponsePacket>) {
    let flow_io = vmess::VmessUdpFlowIo;
    let (send_tx, mut send_rx) = mpsc::channel::<vmess::VmessUdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<UdpResponsePacket>(32);
    let recv_tx_bg = recv_tx.clone();

    proxy.record_session_outbound_tx(session_id, initial_payload_len as u64);

    let proxy_clone = proxy.clone();
    tokio::spawn(async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(packet) => {
                            let (target, port, payload) = packet.into_parts();
                            match flow_io.write_packet(&mut metered, &target, port, &payload).await {
                                Ok(packet_len) => {
                                    proxy_clone.record_session_outbound_tx(session_id, packet_len as u64);
                                }
                                Err(_) => {
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
                read = metered.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            match flow_io.decode_packet(&buffer[..n]) {
                                Ok(packet) => {
                                    let (target, port, payload) = packet.into_parts();
                                    let response = UdpResponsePacket {
                                        target,
                                        port,
                                        payload,
                                    };
                                    if recv_tx_bg.send(response).is_err() {
                                        break;
                                    }
                                }
                                Err(error) => {
                                    tracing::debug!(error = %error, "failed to decode VMess UDP packet");
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
        VmessUdpUpstream {
            session_id,
            send_tx,
        },
        recv_tx,
    )
}

async fn establish_vmess_udp_upstream_over_stream(
    proxy: &Proxy,
    session: &Session,
    identity: vmess::VmessUdpIdentity,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<(VmessUdpUpstream, broadcast::Sender<UdpResponsePacket>), EngineError> {
    let flow_io = vmess::VmessUdpFlowIo;

    let vmess_stream = vmess::establish_udp_flow_stream(stream, session, identity).await?;
    let mut metered = MeteredStream::new(TcpRelayStream::new(vmess_stream));
    let initial_packet_len = flow_io
        .write_packet(&mut metered, &session.target, session.port, initial_payload)
        .await?;

    Ok(spawn_vmess_udp_relay(
        proxy,
        session.id,
        metered,
        initial_packet_len,
    ))
}

async fn establish_vmess_udp_upstream(
    request: &VmessUdpUpstreamRequest<'_>,
) -> Result<(VmessUdpUpstream, broadcast::Sender<UdpResponsePacket>), EngineError> {
    let flow_io = vmess::VmessUdpFlowIo;
    let initial_packet = flow_io.encode_packet(
        &request.session.target,
        request.session.port,
        request.initial_payload,
    )?;

    if let Some(max_concurrency) = request.mux_concurrency {
        let mut mux_stream = request
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
        mux_stream.write_all(&initial_packet).await?;
        tokio::io::AsyncWriteExt::flush(&mut mux_stream).await?;
        let metered = MeteredStream::new(mux_stream);
        return Ok(spawn_vmess_udp_relay(
            request.proxy,
            request.session.id,
            metered,
            initial_packet.len(),
        ));
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

    let vmess_stream =
        vmess::establish_udp_flow_stream(stream, request.session, request.identity).await?;
    let mut metered = MeteredStream::new(TcpRelayStream::new(vmess_stream));
    let initial_packet_len = flow_io
        .write_packet(
            &mut metered,
            &request.session.target,
            request.session.port,
            request.initial_payload,
        )
        .await?;

    Ok(spawn_vmess_udp_relay(
        request.proxy,
        request.session.id,
        metered,
        initial_packet_len,
    ))
}

pub(crate) struct VmessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), (VmessUdpUpstream, broadcast::Sender<UdpResponsePacket>)>,
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
        request: VmessUdpRelayFlow<'_>,
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
        let packet = vmess::VmessUdpFlowPacket::new(target.clone(), port, payload.to_vec());
        let packet_len = packet.encode()?.len() as u64;
        let _ = upstream.send_tx.send(packet).await;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VmessUdpUpstream,
        recv_tx: broadcast::Sender<UdpResponsePacket>,
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
                Ok((packet.target, packet.port, packet.payload, Some(session_id)))
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
            let packet = vmess::VmessUdpFlowPacket::new(
                request.target.clone(),
                request.port,
                request.initial_payload.to_vec(),
            );
            let packet_len = packet.encode()?.len() as u64;
            let _ = upstream.send_tx.send(packet).await;
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
