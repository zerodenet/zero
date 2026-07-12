use super::model::{ManagedUdpHandlers, ManagedUdpState};
use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

impl ManagedUdpState {
    pub(crate) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            datagram: super::super::datagram::ManagedDatagramState::new(handlers.datagram),
            stream: super::super::stream::ManagedStreamState::new(handlers.stream),
            flows: std::collections::HashMap::new(),
            next_flow_id: 1,
        }
    }

    pub(crate) fn register_flow(&mut self, resume: ManagedUdpFlowResume) -> ManagedUdpFlowRef {
        let flow_ref = self.next_flow_ref();
        self.flows.insert(flow_ref, resume);
        flow_ref
    }

    pub(crate) fn flow_resume(&self, flow_ref: ManagedUdpFlowRef) -> Option<ManagedUdpFlowResume> {
        self.flows.get(&flow_ref).cloned()
    }

    fn next_flow_ref(&mut self) -> ManagedUdpFlowRef {
        let flow_ref = ManagedUdpFlowRef::new(self.next_flow_id);
        self.next_flow_id += 1;
        flow_ref
    }
}
