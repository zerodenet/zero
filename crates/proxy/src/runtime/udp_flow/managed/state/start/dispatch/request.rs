use super::super::super::model::ManagedUdpState;
use crate::runtime::udp_flow::managed::flow::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
use crate::runtime::udp_flow::result::FlowFailure;

impl ManagedUdpState {
    pub(crate) async fn start_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<Option<usize>, FlowFailure> {
        match request.kind {
            #[cfg(feature = "managed-datagram-runtime")]
            ManagedUdpFlowKind::Datagram => self.start_datagram_request(request).await,
            #[cfg(feature = "managed-stream-runtime")]
            ManagedUdpFlowKind::StreamPacket => self.start_stream_packet_request(request).await,
            #[cfg(feature = "managed-stream-runtime")]
            ManagedUdpFlowKind::RelayStream => self.start_relay_stream_request(request).await,
        }
    }
}
