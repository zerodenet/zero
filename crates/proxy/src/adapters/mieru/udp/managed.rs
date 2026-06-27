use crate::runtime::udp_flow::managed::{ManagedStreamFlowHandler, ManagedStreamFlowManager};

mod connector;

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(ManagedStreamFlowManager::new(
        connector::MieruManagedStreamConnector,
        "mieru_establish",
        "mieru_relay_upstream",
        "mieru_relay_establish",
        "mieru_relay_send",
        "udp_mieru_resume",
        "expected Mieru UDP flow resume",
    ))
}
