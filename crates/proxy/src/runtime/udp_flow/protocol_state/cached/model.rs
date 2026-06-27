use crate::runtime::udp_flow::managed::ManagedCachedFlowSender;

pub(crate) struct CachedUdpHandlers {
    pub(crate) cached: Vec<Box<dyn ManagedCachedFlowSender>>,
}

pub(in crate::runtime::udp_flow::protocol_state) struct CachedProtocolUdpState {
    handlers: CachedUdpHandlers,
}

impl CachedProtocolUdpState {
    pub(in crate::runtime::udp_flow::protocol_state) fn new(handlers: CachedUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn senders(
        &mut self,
    ) -> impl Iterator<Item = &mut dyn ManagedCachedFlowSender> {
        self.handlers
            .cached
            .iter_mut()
            .map(|handler| handler.as_mut() as &mut dyn ManagedCachedFlowSender)
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn push_sender(
        &mut self,
        sender: Box<dyn ManagedCachedFlowSender>,
    ) {
        self.handlers.cached.push(sender);
    }
}
