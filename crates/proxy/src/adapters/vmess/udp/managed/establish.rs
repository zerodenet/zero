use std::sync::Arc;

use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use super::model::VmessUdpStartFlow;
use crate::adapters::vmess::mux_pool::VmessMuxOpenRequest;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnection, ManagedTupleUdpSender,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn over_stream(
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

pub(super) async fn direct_flow(
    request: &VmessUdpStartFlow<'_>,
) -> Result<ManagedStreamConnection, EngineError> {
    if let Some(max_concurrency) = request.mux_concurrency {
        let mux_identity = request.config.mux_open_identity();
        let mux_stream = request
            .mux_pool
            .open_udp_stream(VmessMuxOpenRequest {
                proxy: request.proxy,
                session: request.session,
                server: request.server.to_owned(),
                port: request.port,
                id: mux_identity.id,
                cipher_name: mux_identity.cipher_name.to_owned(),
                cipher: mux_identity.cipher,
                tls: request.transport.tls,
                ws: request.transport.ws,
                grpc: request.transport.grpc,
                max_concurrency,
            })
            .await?;
        let established = request.config.start_flow_with_initial_packet(
            mux_stream,
            &request.session.target,
            request.session.port,
            request.payload,
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
            request.port,
            request.proxy.resolver.as_ref(),
        )
        .await?;

    let connector = crate::transport::VmessTransportConnector::new(request.transport);
    let stream = connector
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
