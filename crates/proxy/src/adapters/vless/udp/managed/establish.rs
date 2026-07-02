use std::sync::Arc;

use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use super::model::VlessUdpStartFlow;
use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedTupleUdpSender,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn start_mux_fast_path(
    request: &VlessUdpStartFlow<'_>,
) -> Result<bool, EngineError> {
    if !request.config.mux_flow_enabled() {
        return Ok(false);
    }

    let max_concurrency = 8u32;
    let Ok((_mux_sid, up_tx, _down_rx)) = crate::adapters::vless::mux_pool::open_udp_stream(
        request.mux_pool,
        VlessMuxOpenRequest {
            proxy: request.proxy,
            session: None,
            server: request.server,
            port: request.port,
            identity: request.config.mux_pool_identity(),
            tls: request.transport.tls,
            reality: request.transport.reality,
            max_concurrency,
        },
    )
    .await
    else {
        return Ok(false);
    };

    let packet = request.config.mux_initial_flow_packet(
        &request.session.target,
        request.session.port,
        request.payload,
    )?;
    let sent = packet.encoded_len();
    let _ = up_tx.send(packet.into_bytes());
    request
        .proxy
        .record_session_outbound_tx(request.session.id, sent as u64);
    Ok(true)
}

pub(super) async fn over_stream(
    proxy: &Proxy,
    session: &Session,
    config: vless::udp::VlessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<ManagedStreamConnection, EngineError> {
    let established = config
        .establish_flow_with_initial_packet(stream, session, initial_payload)
        .await?;
    proxy.record_session_outbound_tx(session.id, established.initial_packet_len as u64);
    Ok(ManagedStreamConnection::new(
        session.id,
        managed_tuple_udp_connection(Arc::new(established.into_connection())),
    ))
}

pub(super) async fn direct_flow(
    request: &VlessUdpStartFlow<'_>,
) -> Result<ManagedStreamConnection, EngineError> {
    let socket = request
        .proxy
        .protocols
        .direct_connector()
        .connect_host(
            request.server,
            request.port,
            request.proxy.resolver.as_ref(),
        )
        .await?;

    let connector = crate::transport::VlessUdpTransportConnector::new(request.transport);
    let stream: TcpRelayStream = connector
        .connect(socket, request.server, request.port)
        .await?;

    over_stream(
        request.proxy,
        request.session,
        request.config,
        request.payload,
        stream,
    )
    .await
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for vless::udp::VlessUdpFlowConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        vless::udp::VlessUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(EngineError::from)
    }

    fn subscribe_responses(&self) -> vless::udp::VlessUdpFlowResponseReceiver {
        vless::udp::VlessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "vless upstream closed"
    }
}
