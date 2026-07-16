use super::super::model::RegisteredUdpState;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;

impl RegisteredUdpState {
    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        let flow_ref = ManagedUdpFlowRef::new(self.next_managed_flow_id);
        self.next_managed_flow_id += 1;
        self.managed_resumes.insert(flow_ref, resume);
        flow_ref
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.managed_resumes.get(&flow_ref).cloned()
    }
}
