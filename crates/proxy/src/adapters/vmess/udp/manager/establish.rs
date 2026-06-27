use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use super::model::VmessUdpUpstreamRequest;
use crate::adapters::vmess::mux_pool::VmessMuxOpenRequest;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedTupleUdpSender,
};
use crate::transport::TcpRelayStream;
use std::sync::Arc;

struct VmessManagedUdpSender {
    connection: vmess::VmessUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for VmessManagedUdpSender {
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

    fn subscribe_responses(&self) -> vmess::VmessUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "vmess upstream closed"
    }
}

pub(super) async fn over_stream(
    proxy: &crate::runtime::Proxy,
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

pub(super) async fn direct(
    request: &VmessUdpUpstreamRequest<'_>,
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
