use super::model::{ManagedUdpConnection, SharedManagedUdpConnection};
use super::response::spawn_response_bridge;
use crate::runtime::udp_flow::packet_path::ChainTask;
use std::sync::Arc;
use tokio::task::JoinSet;
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;
use zero_transport::managed_udp::ManagedPacketUdpConnectionOps;

#[async_trait::async_trait]
pub(crate) trait ManagedPacketUdpSender: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket>;

    fn closed_message(&self) -> &'static str;
}

struct ManagedPacketUdpConnection {
    sender: Arc<dyn ManagedPacketUdpSender>,
}

#[async_trait::async_trait]
impl ManagedUdpConnection for ManagedPacketUdpConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.sender.send(target, port, payload).await
    }

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64) {
        spawn_response_bridge(
            chain_tasks,
            self.sender.subscribe_responses(),
            session_id,
            self.sender.closed_message(),
            |packet| (packet.target, packet.port, packet.payload),
        );
    }
}

pub(crate) fn managed_packet_udp_connection(
    sender: Arc<dyn ManagedPacketUdpSender>,
) -> SharedManagedUdpConnection {
    Arc::new(ManagedPacketUdpConnection { sender })
}

#[async_trait::async_trait]
pub(crate) trait ManagedPacketUdpFlowConnection: Send + Sync + 'static {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket>;

    fn closed_message(&self) -> &'static str;
}

#[async_trait::async_trait]
impl<T> ManagedPacketUdpFlowConnection for T
where
    T: ManagedPacketUdpConnectionOps,
{
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.send_protocol_packet(target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))
    }

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket> {
        self.subscribe_protocol_packets()
    }

    fn closed_message(&self) -> &'static str {
        self.closed_message_for_connection()
    }
}

struct ManagedPacketUdpFlowSender<T> {
    connection: T,
}

#[async_trait::async_trait]
impl<T> ManagedPacketUdpSender for ManagedPacketUdpFlowSender<T>
where
    T: ManagedPacketUdpFlowConnection,
{
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection.send(target, port, payload).await
    }

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket> {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        self.connection.closed_message()
    }
}

pub(crate) fn managed_packet_udp_connection_from_flow<T>(
    connection: T,
) -> SharedManagedUdpConnection
where
    T: ManagedPacketUdpFlowConnection,
{
    managed_packet_udp_connection(Arc::new(ManagedPacketUdpFlowSender { connection }))
}
