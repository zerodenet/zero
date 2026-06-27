use crate::runtime::udp_flow::managed::{ManagedStreamFlowHandler, ManagedStreamFlowManager};

mod connector;

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(ManagedStreamFlowManager::new(
        connector::TrojanManagedStreamConnector,
        "trojan_establish",
        "trojan_relay_upstream",
        "trojan_relay_establish",
        "trojan_relay_send",
        "udp_trojan_resume",
        "expected Trojan UDP flow resume",
    ))
}
