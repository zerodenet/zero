use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::managed_udp::ManagedTupleUdpConnectionOps;

#[async_trait::async_trait]
pub(crate) trait ManagedTupleUdpFlowConnection: Send + Sync + 'static {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message(&self) -> &'static str;
}

#[async_trait::async_trait]
impl<T> ManagedTupleUdpFlowConnection for T
where
    T: ManagedTupleUdpConnectionOps,
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

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)> {
        self.subscribe_protocol_packets()
    }

    fn closed_message(&self) -> &'static str {
        self.closed_message_for_connection()
    }
}
