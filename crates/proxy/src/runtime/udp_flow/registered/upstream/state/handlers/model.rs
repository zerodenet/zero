use super::super::super::contract::UpstreamUdpHandlers;

pub(in crate::runtime::udp_flow::registered) struct UpstreamAssociationState {
    pub(super) handlers: UpstreamUdpHandlers,
}

impl UpstreamAssociationState {
    pub(in crate::runtime::udp_flow::registered) fn new(handlers: UpstreamUdpHandlers) -> Self {
        Self { handlers }
    }
}
