use super::connection::managed_packet_udp_connection;
use super::flow::ManagedPacketUdpFlowConnection;
use super::sender::ManagedPacketUdpSender;
use crate::runtime::udp_flow::managed::connection::model::SharedManagedUdpConnection;
use std::sync::Arc;
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;

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
