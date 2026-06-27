use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedTupleUdpSender,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;

struct VlessManagedUdpSender {
    connection: vless::VlessUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for VlessManagedUdpSender {
    async fn send(
        &self,
        target: &zero_core::Address,
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

pub(super) async fn over_stream(
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

pub(super) async fn direct(
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
