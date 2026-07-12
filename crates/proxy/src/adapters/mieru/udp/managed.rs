use crate::runtime::udp_flow::managed::{
    bridge::{managed_stream_handler_box, ManagedStreamStages},
    ManagedStreamFlowHandler,
};

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    managed_stream_handler_box::<zero_transport::mieru_transport::MieruManagedStreamUdpResume>(
        ManagedStreamStages::from_resume::<
            zero_transport::mieru_transport::MieruManagedStreamUdpResume,
        >(),
    )
}
