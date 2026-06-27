use std::sync::Arc;

use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::adapters::vmess::mux_pool::{VmessMuxConnectionPool, VmessMuxOpenRequest};
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedStreamConnectionSend,
    ManagedStreamFlowSender, ManagedStreamPacketSender, ManagedTupleUdpSender,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) struct VmessUdpOutboundManager {
    upstreams: ManagedStreamPacketSender,
}

impl VmessUdpOutboundManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedStreamPacketSender::new(),
        }
    }

    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        request: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.upstreams
            .send_or_insert_target(
                &request.session.target,
                request.session.port,
                ManagedStreamConnectionSend {
                    chain_tasks,
                    proxy: request.proxy,
                    target: &request.session.target,
                    port: request.session.port,
                    payload: request.payload,
                },
                direct(VmessUdpUpstreamRequest {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    server_port: request.port,
                    config: request.config,
                    mux_pool: request.mux_pool,
                    initial_payload: request.payload,
                    transport: Some(&request.transport),
                    mux_concurrency: request.mux_concurrency,
                }),
            )
            .await
    }

    pub(crate) async fn start_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        request: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vmess_outbound_transport_over_stream(
            crate::transport::VmessFinalHopTransportRequest {
                carrier: request.carrier,
                options: request.transport,
            },
        )
        .await?;
        let upstream = over_stream(
            request.proxy,
            request.session,
            request.config,
            request.payload,
            stream,
        )
        .await?;
        self.upstreams.insert_and_bridge_target(
            request.session.target.clone(),
            request.session.port,
            chain_tasks,
            upstream,
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowSender for VmessUdpOutboundManager {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.upstreams
            .send_existing_target(target, port, chain_tasks, proxy, payload)
            .await
    }
}

pub(crate) struct VmessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a VmessMuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vmess::VmessUdpFlowConfig<'a>,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessUdpRelayFlowStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vmess::VmessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

struct VmessUdpUpstreamRequest<'a> {
    proxy: &'a Proxy,
    mux_pool: &'a VmessMuxConnectionPool,
    session: &'a Session,
    server: &'a str,
    server_port: u16,
    config: vmess::VmessUdpFlowConfig<'a>,
    initial_payload: &'a [u8],
    transport: Option<&'a crate::transport::VmessTransportOptions<'a>>,
    mux_concurrency: Option<u32>,
}

async fn over_stream(
    proxy: &Proxy,
    session: &Session,
    config: vmess::VmessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<ManagedStreamConnection, EngineError> {
    let established = config
        .establish_flow_with_initial_packet(stream, session, initial_payload)
        .await?;
    proxy.record_session_outbound_tx(session.id, established.initial_packet_len as u64);
    Ok(ManagedStreamConnection::new(
        session.id,
        managed_tuple_udp_connection(Arc::new(VmessManagedUdpSender {
            connection: established.into_connection(),
        })),
    ))
}

async fn direct(
    request: VmessUdpUpstreamRequest<'_>,
) -> Result<ManagedStreamConnection, EngineError> {
    if let Some(max_concurrency) = request.mux_concurrency {
        let mux_stream = request
            .mux_pool
            .open_udp_stream(VmessMuxOpenRequest {
                proxy: request.proxy,
                session: request.session,
                server: request.server.to_owned(),
                port: request.server_port,
                id: request.config.uuid(),
                cipher_name: request.config.cipher_name().to_owned(),
                cipher: request.config.cipher(),
                tls: request.transport.and_then(|transport| transport.tls),
                ws: request.transport.and_then(|transport| transport.ws),
                grpc: request.transport.and_then(|transport| transport.grpc),
                max_concurrency,
            })
            .await?;
        let established = request.config.start_flow_with_initial_packet(
            mux_stream,
            &request.session.target,
            request.session.port,
            request.initial_payload,
        )?;
        request
            .proxy
            .record_session_outbound_tx(request.session.id, established.initial_packet_len as u64);
        return Ok(ManagedStreamConnection::new(
            request.session.id,
            managed_tuple_udp_connection(Arc::new(VmessManagedUdpSender {
                connection: established.into_connection(),
            })),
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

    over_stream(
        request.proxy,
        request.session,
        request.config,
        request.initial_payload,
        stream,
    )
    .await
}

struct VmessManagedUdpSender {
    connection: vmess::VmessUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for VmessManagedUdpSender {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send(target, port, payload)
            .await
            .map_err(EngineError::from)
    }

    fn subscribe_responses(&self) -> vmess::VmessUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "vmess upstream closed"
    }
}
