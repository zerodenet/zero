use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;

#[async_trait::async_trait]
pub(crate) trait ManagedPacketUdpFlowConnection: Send + Sync + 'static {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket>;

    fn closed_message(&self) -> &'static str;
}
