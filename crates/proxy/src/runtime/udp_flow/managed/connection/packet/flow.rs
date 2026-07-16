use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;
use zero_transport::managed_udp::ManagedPacketUdpConnectionOps;

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
