//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

mod model;

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use vmess::{parse_uuid, VmessCipher};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_traits::{AsyncSocket, UdpPacketFraming};

pub(crate) use model::{VmessUdpRelayFlow, VmessUdpStartFlow, VmessUdpTransport};

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
use model::{VmessUdpUpstream, VmessUdpUpstreamRequest};

fn spawn_vmess_udp_relay(
    proxy: &Proxy,
    session_id: u64,
    mut metered: MeteredStream<TcpRelayStream>,
    initial_payload_len: usize,
) -> (VmessUdpUpstream, broadcast::Sender<vmess::VmessUdpPacket>) {
    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, _) = broadcast::channel::<vmess::VmessUdpPacket>(32);
    let recv_tx_bg = recv_tx.clone();
    let vmess_outbound = proxy.protocols.vmess_outbound_protocol();

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
                            match <vmess::VmessOutbound as UdpPacketFraming<
                                vmess::VmessUdpPacketTarget,
                            >>::decode_udp_packet(&vmess_outbound, &buffer[..n]) {
                                Ok(packet) => {
                                    if recv_tx_bg.send(packet).is_err() {
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

async fn build_vmess_udp_transport_over_stream(
    stream: TcpRelayStream,
    transport: Option<&VmessUdpTransport<'_>>,
    source_dir: Option<&std::path::Path>,
    server: &str,
    port: u16,
) -> Result<TcpRelayStream, EngineError> {
    match transport {
        Some(VmessUdpTransport {
            grpc: Some(grpc_cfg),
            ws: None,
            tls: Some(tls_cfg),
        }) => {
            let tls_stream =
                zero_transport::tls::connect_tls_stream(stream, tls_cfg, source_dir, server)
                    .await?;
            Ok(TcpRelayStream::new(
                zero_transport::grpc::connect_grpc(tls_stream, &grpc_cfg.service_names).await?,
            ))
        }
        Some(VmessUdpTransport {
            grpc: Some(grpc_cfg),
            ws: None,
            tls: None,
        }) => Ok(TcpRelayStream::new(
            zero_transport::grpc::connect_grpc(stream, &grpc_cfg.service_names).await?,
        )),
        Some(VmessUdpTransport {
            grpc: None,
            ws: Some(ws_cfg),
            tls: Some(tls_cfg),
        }) => {
            let tls_stream =
                zero_transport::tls::connect_tls_stream(stream, tls_cfg, source_dir, server)
                    .await?;
            Ok(TcpRelayStream::new(
                zero_transport::ws::connect_ws(tls_stream, ws_cfg, server, port).await?,
            ))
        }
        Some(VmessUdpTransport {
            grpc: None,
            ws: Some(ws_cfg),
            tls: None,
        }) => Ok(TcpRelayStream::new(
            zero_transport::ws::connect_ws(stream, ws_cfg, server, port).await?,
        )),
        Some(VmessUdpTransport {
            grpc: None,
            ws: None,
            tls: Some(tls_cfg),
        }) => zero_transport::tls::connect_tls_stream(stream, tls_cfg, source_dir, server).await,
        Some(VmessUdpTransport {
            grpc: None,
            ws: None,
            tls: None,
        })
        | None => Ok(stream),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "vmess: ws and grpc are mutually exclusive",
        ))),
    }
}

async fn establish_vmess_udp_upstream_over_stream(
    proxy: &Proxy,
    session: &Session,
    id: &str,
    cipher: &str,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<(VmessUdpUpstream, broadcast::Sender<vmess::VmessUdpPacket>), EngineError> {
    let uuid = parse_uuid(id)?;
    let vmess_cipher = VmessCipher::from_name(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {cipher}"),
        ))
    })?;
    let initial_packet =
        <vmess::VmessOutbound as UdpPacketFraming<vmess::VmessUdpPacketTarget>>::encode_udp_packet(
            &proxy.protocols.vmess_outbound_protocol(),
            &vmess::VmessUdpPacketTarget {
                address: &session.target,
                port: session.port,
                payload: initial_payload,
            },
        )?;

    let vmess_stream = vmess::VmessAeadStream::establish_udp_outbound(
        stream,
        &proxy.protocols.vmess_outbound_protocol(),
        session,
        &uuid,
        vmess_cipher,
    )
    .await?;
    let mut metered = MeteredStream::new(TcpRelayStream::new(vmess_stream));
    metered.write_all(&initial_packet).await?;

    Ok(spawn_vmess_udp_relay(
        proxy,
        session.id,
        metered,
        initial_packet.len(),
    ))
}

async fn establish_vmess_udp_upstream(
    request: &VmessUdpUpstreamRequest<'_>,
) -> Result<(VmessUdpUpstream, broadcast::Sender<vmess::VmessUdpPacket>), EngineError> {
    let uuid = parse_uuid(request.id)?;
    let vmess_cipher = VmessCipher::from_name(request.cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {}", request.cipher),
        ))
    })?;
    let initial_packet =
        <vmess::VmessOutbound as UdpPacketFraming<vmess::VmessUdpPacketTarget>>::encode_udp_packet(
            &request.proxy.protocols.vmess_outbound_protocol(),
            &vmess::VmessUdpPacketTarget {
                address: &request.session.target,
                port: request.session.port,
                payload: request.initial_payload,
            },
        )?;

    if let Some(max_concurrency) = request.mux_concurrency {
        let mut mux_stream = request
            .proxy
            .vmess_mux_pool
            .open_udp_stream(
                crate::protocol_runtime::vmess_mux_pool::VmessMuxOpenRequest {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server.to_owned(),
                    port: request.server_port,
                    id: uuid,
                    cipher: request.cipher.to_owned(),
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

    let stream: TcpRelayStream = match request.transport {
        Some(VmessUdpTransport {
            grpc: Some(grpc_cfg),
            ws: None,
            tls: Some(tls_cfg),
        }) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                request.proxy.config.source_dir(),
                request.server,
            )
            .await?;
            TcpRelayStream::new(
                zero_transport::grpc::connect_grpc(tls_stream, &grpc_cfg.service_names).await?,
            )
        }
        Some(VmessUdpTransport {
            grpc: Some(grpc_cfg),
            ws: None,
            tls: None,
        }) => TcpRelayStream::new(
            zero_transport::grpc::connect_grpc(socket, &grpc_cfg.service_names).await?,
        ),
        Some(VmessUdpTransport {
            grpc: None,
            ws: Some(ws_cfg),
            tls: Some(tls_cfg),
        }) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                request.proxy.config.source_dir(),
                request.server,
            )
            .await?;
            TcpRelayStream::new(
                zero_transport::ws::connect_ws(
                    tls_stream,
                    ws_cfg,
                    request.server,
                    request.server_port,
                )
                .await?,
            )
        }
        Some(VmessUdpTransport {
            grpc: None,
            ws: Some(ws_cfg),
            tls: None,
        }) => TcpRelayStream::new(
            zero_transport::ws::connect_ws(socket, ws_cfg, request.server, request.server_port)
                .await?,
        ),
        Some(VmessUdpTransport {
            grpc: None,
            ws: None,
            tls: Some(tls_cfg),
        }) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                request.proxy.config.source_dir(),
                request.server,
            )
            .await?;
            TcpRelayStream::new(tls_stream)
        }
        Some(VmessUdpTransport {
            grpc: None,
            ws: None,
            tls: None,
        })
        | None => TcpRelayStream::new(socket),
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vmess: ws and grpc are mutually exclusive",
            )))
        }
    };

    let vmess_stream = vmess::VmessAeadStream::establish_udp_outbound(
        stream,
        &request.proxy.protocols.vmess_outbound_protocol(),
        request.session,
        &uuid,
        vmess_cipher,
    )
    .await?;
    let mut metered = MeteredStream::new(TcpRelayStream::new(vmess_stream));
    metered.write_all(&initial_packet).await?;

    Ok(spawn_vmess_udp_relay(
        request.proxy,
        request.session.id,
        metered,
        initial_packet.len(),
    ))
}

pub(crate) struct VmessUdpOutboundManager {
    upstreams:
        HashMap<(Address, u16), (VmessUdpUpstream, broadcast::Sender<vmess::VmessUdpPacket>)>,
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
                id: request.id,
                cipher: request.cipher,
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
        let stream = build_vmess_udp_transport_over_stream(
            request.carrier.stream,
            Some(&request.transport),
            request.proxy.config.source_dir(),
            request.server,
            request.port,
        )
        .await?;
        let (upstream, recv_tx) = establish_vmess_udp_upstream_over_stream(
            request.proxy,
            request.session,
            request.id,
            request.cipher,
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
        let packet = <vmess::VmessOutbound as UdpPacketFraming<
            vmess::VmessUdpPacketTarget,
        >>::encode_udp_packet(
            &proxy.protocols.vmess_outbound_protocol(),
            &vmess::VmessUdpPacketTarget {
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

    fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VmessUdpUpstream,
        recv_tx: broadcast::Sender<vmess::VmessUdpPacket>,
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
            let packet = <vmess::VmessOutbound as UdpPacketFraming<
                vmess::VmessUdpPacketTarget,
            >>::encode_udp_packet(
                &request.proxy.protocols.vmess_outbound_protocol(),
                &vmess::VmessUdpPacketTarget {
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
