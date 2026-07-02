use std::sync::Arc;

use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedTupleUdpSender,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn over_stream(
    proxy: &Proxy,
    session: &Session,
    config: vmess::udp::VmessUdpFlowConfig<'_>,
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

#[allow(clippy::too_many_arguments)]
pub(super) async fn direct_flow(
    proxy: &Proxy,
    mux_pool: &vmess::mux::VmessMuxConnectionPool,
    session: &Session,
    server: &str,
    port: u16,
    config: vmess::udp::VmessUdpFlowConfig<'_>,
    mux_concurrency: Option<u32>,
    transport: crate::transport::VmessTransportOptions<'_>,
    payload: &[u8],
) -> Result<ManagedStreamConnection, EngineError> {
    if let Some(max_concurrency) = mux_concurrency {
        let mux_stream = crate::adapters::vmess::mux_pool::open_udp_stream(
            mux_pool,
            proxy,
            session,
            server,
            port,
            config.mux_pool_identity(),
            transport.tls,
            transport.ws,
            transport.grpc,
            max_concurrency,
        )
        .await?;
        let established = config.start_flow_with_initial_packet(
            mux_stream,
            &session.target,
            session.port,
            payload,
        )?;
        proxy.record_session_outbound_tx(session.id, established.initial_packet_len as u64);
        return Ok(ManagedStreamConnection::new(
            session.id,
            managed_tuple_udp_connection(Arc::new(established.into_connection())),
        ));
    }

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let connector = crate::transport::VmessTransportConnector::new(transport);
    let stream = connector.connect(socket, server, port).await?;

    over_stream(proxy, session, config, payload, stream).await
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for vmess::udp::VmessUdpFlowConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        vmess::udp::VmessUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(EngineError::from)
    }

    fn subscribe_responses(&self) -> vmess::udp::VmessUdpFlowResponseReceiver {
        vmess::udp::VmessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "vmess upstream closed"
    }
}
