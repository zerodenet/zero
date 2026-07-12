use super::super::super::connection::SharedManagedDatagramUdpConnection;
use super::super::response::ManagedDatagramResponse;
use super::managed_datagram_connection_from_flow;
use super::model::ManagedDatagramFlowConnection;
use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::managed_udp::ManagedDatagramConnectionOps;

struct ManagedDatagramOpsConnection<T> {
    connection: T,
}

#[async_trait::async_trait]
impl<T> ManagedDatagramFlowConnection for ManagedDatagramOpsConnection<T>
where
    T: ManagedDatagramConnectionOps,
{
    async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.connection
            .send_protocol_datagram(target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))
    }

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<ManagedDatagramResponse> {
        self.connection.subscribe_protocol_datagrams()
    }

    fn closed_message(&self) -> &'static str {
        self.connection.closed_message_for_datagram_connection()
    }
}

pub(crate) fn managed_datagram_connection_from_ops<T>(
    connection: T,
) -> SharedManagedDatagramUdpConnection
where
    T: ManagedDatagramConnectionOps,
{
    managed_datagram_connection_from_flow(ManagedDatagramOpsConnection { connection })
}
