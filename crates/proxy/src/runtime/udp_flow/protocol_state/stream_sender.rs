use super::ProtocolUdpState;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

pub(super) mod model;

pub(crate) use model::ManagedStreamFlowSender;
pub(crate) use model::ManagedStreamSenderHandlers;
pub(super) use model::ManagedStreamSenderState;

impl ProtocolUdpState {
    pub(crate) fn register_managed_stream_flow_sender(
        &mut self,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) -> ManagedUdpFlowRef {
        let flow_ref = self.next_managed_flow_ref();
        self.stream_senders.push_sender(flow_ref, sender);
        flow_ref
    }

    pub(super) fn has_stream_flow_sender(&self, flow_ref: ManagedUdpFlowRef) -> bool {
        self.stream_senders.contains_sender(flow_ref)
    }
}
