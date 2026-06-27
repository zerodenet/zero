use crate::protocol_runtime::udp::state::managed::model::ManagedCachedFlowSender;
use crate::protocol_runtime::udp::CachedUdpFlowHandler;

pub(crate) struct CachedUdpHandlers {
    pub(crate) cached: Vec<Box<dyn CachedUdpFlowHandler>>,
}

pub(in crate::protocol_runtime::udp::state) struct CachedProtocolUdpState {
    handlers: CachedUdpHandlers,
}

impl CachedProtocolUdpState {
    pub(in crate::protocol_runtime::udp::state) fn new(handlers: CachedUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state) fn senders(
        &mut self,
    ) -> impl Iterator<Item = &mut dyn ManagedCachedFlowSender> {
        self.handlers
            .cached
            .iter_mut()
            .map(|handler| handler.as_mut() as &mut dyn ManagedCachedFlowSender)
    }

    pub(in crate::protocol_runtime::udp::state) fn handlers(
        &mut self,
    ) -> impl Iterator<Item = &mut (dyn CachedUdpFlowHandler + '_)> + '_ {
        self.handlers
            .cached
            .iter_mut()
            .map(move |handler| handler.as_mut() as &mut (dyn CachedUdpFlowHandler + '_))
    }
}
