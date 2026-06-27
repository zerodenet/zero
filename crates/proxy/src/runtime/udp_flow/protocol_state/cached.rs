use super::ProtocolUdpState;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

pub(super) mod model;

pub(crate) use model::CachedProtocolFlowSender;
pub(super) use model::CachedProtocolUdpState;
pub(crate) use model::CachedUdpHandlers;

impl ProtocolUdpState {
    pub(crate) fn register_cached_flow_sender(
        &mut self,
        sender: Box<dyn CachedProtocolFlowSender>,
    ) -> ManagedUdpFlowRef {
        let flow_ref = self.next_managed_flow_ref();
        self.cached.push_sender(flow_ref, sender);
        flow_ref
    }

    pub(super) fn has_cached_flow_sender(&self, flow_ref: ManagedUdpFlowRef) -> bool {
        self.cached.contains_sender(flow_ref)
    }
}
