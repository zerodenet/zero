use super::super::state::ProtocolUdpState;
use super::super::{FlowFailure, ManagedRelayStreamFlow, ManagedStreamPacketFlow};

impl ProtocolUdpState {
    pub(crate) async fn start_managed_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.managed.start_stream_packet_flow(request).await
    }

    pub(crate) async fn start_managed_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.managed.start_relay_stream_flow(request).await
    }
}
