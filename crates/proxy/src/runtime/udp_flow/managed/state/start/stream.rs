use super::super::model::ManagedUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::{ManagedRelayStreamFlow, ManagedStreamPacketFlow};

impl ManagedUdpState {
    pub(crate) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_stream_packet_flow(request).await
    }

    pub(crate) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.stream.start_relay_stream_flow(request).await
    }
}
