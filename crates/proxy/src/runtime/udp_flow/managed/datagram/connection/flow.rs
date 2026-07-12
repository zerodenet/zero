use std::sync::Arc;

use super::super::super::connection::SharedManagedDatagramUdpConnection;
use super::model::{ManagedDatagramFlowConnection, ManagedDatagramSender};
use super::sender::managed_datagram_connection;
use zero_core::Address;
use zero_engine::EngineError;

struct ManagedDatagramFlowSender<T> {
    connection: T,
}

#[async_trait::async_trait]
impl<T> ManagedDatagramSender for ManagedDatagramFlowSender<T>
where
    T: ManagedDatagramFlowConnection,
{
    async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.connection.send_datagram(target, port, payload).await
    }
}

pub(crate) fn managed_datagram_connection_from_flow<T>(
    connection: T,
) -> SharedManagedDatagramUdpConnection
where
    T: ManagedDatagramFlowConnection,
{
    let response_rx = connection.subscribe_responses();
    let closed_message = connection.closed_message();
    managed_datagram_connection(
        Arc::new(ManagedDatagramFlowSender { connection }),
        response_rx,
        closed_message,
    )
}
