use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedTupleUdpSender, SharedManagedUdpConnection,
};
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use zero_engine::EngineError;

struct Hysteria2ManagedUdpSender {
    connection: hysteria2::Hysteria2UdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for Hysteria2ManagedUdpSender {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send(target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))
    }

    fn subscribe_responses(&self) -> hysteria2::Hysteria2UdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "h2 upstream closed"
    }
}

pub(super) async fn upstream(
    endpoint: OutboundEndpoint<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
    initial_packet: UdpPacketRef<'_>,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let session =
        crate::outbound::hysteria2::establish_udp_flow_session(endpoint, initial_packet, resume)
            .await?;

    Ok(managed_tuple_udp_connection(Arc::new(
        Hysteria2ManagedUdpSender {
            connection: session,
        },
    )))
}
