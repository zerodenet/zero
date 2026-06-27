use super::connect;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedTupleUdpSender, SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use zero_engine::EngineError;

struct MieruManagedUdpSender {
    connection: mieru::MieruUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for MieruManagedUdpSender {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send(target, port, payload)
            .await
            .map_err(|error| {
                EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}")))
            })
    }

    fn subscribe_responses(&self) -> mieru::MieruUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "mieru upstream closed"
    }
}

pub(super) async fn direct(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let stream = connect::direct_stream(proxy, endpoint).await?;
    packet_stream(stream, resume).await
}

pub(super) async fn packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = mieru::establish_udp_flow_with_resume(stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    Ok(managed_tuple_udp_connection(Arc::new(
        MieruManagedUdpSender { connection },
    )))
}
