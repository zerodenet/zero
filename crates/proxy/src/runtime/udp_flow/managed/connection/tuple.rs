use super::model::{ManagedUdpConnection, SharedManagedUdpConnection};
use super::response::spawn_tuple_response_bridge;
use crate::runtime::udp_flow::packet_path::ChainTask;
use std::sync::Arc;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::managed_udp::ManagedTupleUdpConnectionOps;

#[async_trait::async_trait]
pub(crate) trait ManagedTupleUdpSender: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message(&self) -> &'static str;
}

struct ManagedTupleUdpConnection {
    sender: Arc<dyn ManagedTupleUdpSender>,
}

#[async_trait::async_trait]
impl ManagedUdpConnection for ManagedTupleUdpConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.sender.send(target, port, payload).await
    }

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64) {
        spawn_tuple_response_bridge(
            chain_tasks,
            self.sender.subscribe_responses(),
            session_id,
            self.sender.closed_message(),
        );
    }
}

pub(crate) fn managed_tuple_udp_connection(
    sender: Arc<dyn ManagedTupleUdpSender>,
) -> SharedManagedUdpConnection {
    Arc::new(ManagedTupleUdpConnection { sender })
}

#[async_trait::async_trait]
pub(crate) trait ManagedTupleUdpFlowConnection: Send + Sync + 'static {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message(&self) -> &'static str;
}

struct ManagedTupleUdpOpsConnection<T> {
    connection: T,
}

#[async_trait::async_trait]
impl<T> ManagedTupleUdpFlowConnection for ManagedTupleUdpOpsConnection<T>
where
    T: ManagedTupleUdpConnectionOps,
{
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send_protocol_packet(target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))
    }

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)> {
        self.connection.subscribe_protocol_packets()
    }

    fn closed_message(&self) -> &'static str {
        self.connection.closed_message_for_connection()
    }
}

struct ManagedTupleUdpFlowSender<T> {
    connection: T,
}

#[async_trait::async_trait]
impl<T> ManagedTupleUdpSender for ManagedTupleUdpFlowSender<T>
where
    T: ManagedTupleUdpFlowConnection,
{
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection.send(target, port, payload).await
    }

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)> {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        self.connection.closed_message()
    }
}

pub(crate) fn managed_tuple_udp_connection_from_flow<T>(connection: T) -> SharedManagedUdpConnection
where
    T: ManagedTupleUdpFlowConnection,
{
    managed_tuple_udp_connection(Arc::new(ManagedTupleUdpFlowSender { connection }))
}

pub(crate) fn managed_tuple_udp_connection_from_ops<T>(connection: T) -> SharedManagedUdpConnection
where
    T: ManagedTupleUdpConnectionOps,
{
    managed_tuple_udp_connection_from_flow(ManagedTupleUdpOpsConnection { connection })
}
