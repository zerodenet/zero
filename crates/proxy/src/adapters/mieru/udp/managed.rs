use crate::runtime::udp_flow::managed::{
    managed_stream_handler_box, ManagedStreamFlowHandler, ManagedStreamStages,
};

mod connector;

const MIERU_UDP_STAGES: ManagedStreamStages = ManagedStreamStages {
    establish_stage: "mieru_establish",
    relay_upstream_stage: "mieru_relay_upstream",
    relay_establish_stage: "mieru_relay_establish",
    relay_send_stage: "mieru_relay_send",
    mismatch_stage: "udp_mieru_resume",
    mismatch_message: "expected Mieru UDP flow resume",
};

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    managed_stream_handler_box::<mieru::udp::MieruUdpFlowResume>(MIERU_UDP_STAGES)
}
