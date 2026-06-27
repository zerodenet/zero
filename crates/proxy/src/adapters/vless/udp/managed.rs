use std::sync::Arc;

use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::adapters::vless::mux_pool::{MuxConnectionPool, VlessMuxOpenRequest};
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedStreamConnectionSend,
    ManagedStreamFlowSender, ManagedStreamPacketSender, ManagedTupleUdpSender,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) struct VlessUdpOutboundManager {
    upstreams: ManagedStreamPacketSender,
}

impl VlessUdpOutboundManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedStreamPacketSender::new(),
        }
    }

    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        request: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        if start_mux_fast_path(&request).await? {
            return Ok(());
        }

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
                direct(
                    request.proxy,
                    request.session,
                    request.server,
                    request.port,
                    request.config,
                    request.payload,
                    Some(&request.transport),
                ),
            )
            .await
    }

    pub(crate) async fn start_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        request: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vless_split_http_over_relay(
            request.post_carrier.stream,
            request.get_carrier.stream,
            request.split_http,
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

    pub(crate) async fn start_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
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
impl ManagedStreamFlowSender for VlessUdpOutboundManager {
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

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a MuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
    pub(crate) split_http: &'a zero_config::SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHopStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

async fn start_mux_fast_path(request: &VlessUdpStartFlow<'_>) -> Result<bool, EngineError> {
    if !request.config.mux_flow_enabled() {
        return Ok(false);
    }

    let max_concurrency = 8u32;
    let Ok((_mux_sid, up_tx, _down_rx)) = request
        .mux_pool
        .open_udp_stream(VlessMuxOpenRequest {
            proxy: request.proxy,
            session: None,
            server: request.server,
            port: request.port,
            id: request.config.uuid(),
            tls: request.transport.tls,
            reality: request.transport.reality,
            max_concurrency,
        })
        .await
    else {
        return Ok(false);
    };

    let packet = request.config.encode_initial_flow_packet(
        &request.session.target,
        request.session.port,
        request.payload,
    )?;
    let sent = packet.len();
    let _ = up_tx.send(packet);
    request
        .proxy
        .record_session_outbound_tx(request.session.id, sent as u64);
    Ok(true)
}

async fn over_stream(
    proxy: &Proxy,
    session: &Session,
    config: vless::VlessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<ManagedStreamConnection, EngineError> {
    let established = config
        .establish_flow_with_initial_packet(stream, session, initial_payload)
        .await?;
    proxy.record_session_outbound_tx(session.id, established.initial_packet_len as u64);
    Ok(ManagedStreamConnection::new(
        session.id,
        managed_tuple_udp_connection(Arc::new(VlessManagedUdpSender {
            connection: established.into_connection(),
        })),
    ))
}

async fn direct(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    config: vless::VlessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    transport: Option<&crate::transport::VlessUdpTransportOptions<'_>>,
) -> Result<ManagedStreamConnection, EngineError> {
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

    over_stream(proxy, session, config, initial_payload, stream).await
}

struct VlessManagedUdpSender {
    connection: vless::VlessUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for VlessManagedUdpSender {
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

    fn subscribe_responses(&self) -> vless::VlessUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "vless upstream closed"
    }
}
