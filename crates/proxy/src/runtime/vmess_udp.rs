//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use vmess::{parse_uuid, VmessCipher};
use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_traits::{AsyncSocket, UdpPacketFraming};

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

#[derive(Clone)]
pub(super) struct VmessUdpUpstream {
    pub(crate) session_id: u64,
    pub(crate) send_tx: mpsc::Sender<Vec<u8>>,
}

#[derive(Clone, Copy)]
pub(super) struct VmessUdpTransport<'a> {
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
}

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
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    cipher: &str,
    initial_payload: &[u8],
    transport: Option<&VmessUdpTransport<'_>>,
    mux_concurrency: Option<u32>,
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

    if let Some(max_concurrency) = mux_concurrency {
        let mut mux_stream = proxy
            .vmess_mux_pool
            .open_udp_stream(crate::runtime::vmess_mux_pool::VmessMuxOpenRequest {
                proxy,
                session,
                server: server.to_owned(),
                port,
                id: uuid,
                cipher: cipher.to_owned(),
                tls: transport.and_then(|transport| transport.tls),
                ws: transport.and_then(|transport| transport.ws),
                grpc: transport.and_then(|transport| transport.grpc),
                max_concurrency,
            })
            .await?;
        mux_stream.write_all(&initial_packet).await?;
        tokio::io::AsyncWriteExt::flush(&mut mux_stream).await?;
        let metered = MeteredStream::new(mux_stream);
        return Ok(spawn_vmess_udp_relay(
            proxy,
            session.id,
            metered,
            initial_packet.len(),
        ));
    }

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream: TcpRelayStream = match transport {
        Some(VmessUdpTransport {
            grpc: Some(grpc_cfg),
            ws: None,
            tls: Some(tls_cfg),
        }) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                proxy.config.source_dir(),
                server,
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
                proxy.config.source_dir(),
                server,
            )
            .await?;
            TcpRelayStream::new(
                zero_transport::ws::connect_ws(tls_stream, ws_cfg, server, port).await?,
            )
        }
        Some(VmessUdpTransport {
            grpc: None,
            ws: Some(ws_cfg),
            tls: None,
        }) => {
            TcpRelayStream::new(zero_transport::ws::connect_ws(socket, ws_cfg, server, port).await?)
        }
        Some(VmessUdpTransport {
            grpc: None,
            ws: None,
            tls: Some(tls_cfg),
        }) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                proxy.config.source_dir(),
                server,
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

pub(super) struct VmessUdpOutboundManager {
    upstreams:
        HashMap<(Address, u16), (VmessUdpUpstream, broadcast::Sender<vmess::VmessUdpPacket>)>,
}

impl VmessUdpOutboundManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(super) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        tls: Option<&ClientTlsConfig>,
        ws: Option<&WebSocketConfig>,
        grpc: Option<&GrpcConfig>,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let transport = VmessUdpTransport { tls, ws, grpc };
        self.get_or_create_upstream(
            chain_tasks,
            proxy,
            session,
            session.target.clone(),
            session.port,
            server.to_string(),
            port,
            id.to_string(),
            cipher.to_string(),
            payload.to_vec(),
            Some(&transport),
            mux_concurrency,
        )
        .await
    }

    pub(super) async fn start_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        tls: Option<&ClientTlsConfig>,
        ws: Option<&WebSocketConfig>,
        grpc: Option<&GrpcConfig>,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let transport = VmessUdpTransport { tls, ws, grpc };
        let stream = build_vmess_udp_transport_over_stream(
            carrier.stream,
            Some(&transport),
            proxy.config.source_dir(),
            server,
            port,
        )
        .await?;
        let (upstream, recv_tx) =
            establish_vmess_udp_upstream_over_stream(proxy, session, id, cipher, payload, stream)
                .await?;
        self.insert_upstream((session.target.clone(), session.port), upstream, recv_tx);
        self.spawn_bridge(
            chain_tasks,
            session.target.clone(),
            session.port,
            session.id,
        );
        Ok(())
    }

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

    pub(super) fn insert_upstream(
        &mut self,
        key: (Address, u16),
        upstream: VmessUdpUpstream,
        recv_tx: broadcast::Sender<vmess::VmessUdpPacket>,
    ) {
        self.upstreams.insert(key, (upstream, recv_tx));
    }

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
                    .map_err(|_| EngineError::Io(std::io::Error::other("vmess upstream closed")))?;
                Ok((packet.target, packet.port, packet.payload, Some(session_id)))
            });
        }
    }

    pub(super) async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_dispatch::ChainTask>,
        proxy: &Proxy,
        session: &Session,
        target: Address,
        port: u16,
        server: String,
        server_port: u16,
        id: String,
        cipher: String,
        initial_payload: Vec<u8>,
        transport: Option<&VmessUdpTransport<'_>>,
        mux_concurrency: Option<u32>,
    ) -> Result<(), EngineError> {
        let key = (target.clone(), port);
        if let Some((upstream, _)) = self.upstreams.get(&key) {
            proxy.record_session_inbound_rx(upstream.session_id, initial_payload.len() as u64);
            let packet = <vmess::VmessOutbound as UdpPacketFraming<
                vmess::VmessUdpPacketTarget,
            >>::encode_udp_packet(
                &proxy.protocols.vmess_outbound_protocol(),
                &vmess::VmessUdpPacketTarget {
                    address: &target,
                    port,
                    payload: &initial_payload,
                },
            )?;
            let packet_len = packet.len() as u64;
            let _ = upstream.send_tx.send(packet).await;
            proxy.record_session_outbound_tx(upstream.session_id, packet_len);
            self.spawn_bridge(chain_tasks, target, port, upstream.session_id);
            return Ok(());
        }

        let (upstream, recv_tx) = establish_vmess_udp_upstream(
            proxy,
            session,
            &server,
            server_port,
            &id,
            &cipher,
            &initial_payload,
            transport,
            mux_concurrency,
        )
        .await?;
        let session_id = upstream.session_id;
        self.upstreams.insert(key, (upstream, recv_tx));
        self.spawn_bridge(chain_tasks, target, port, session_id);
        Ok(())
    }
}
