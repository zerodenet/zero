use crate::protocol_runtime::udp::state::managed::model::ManagedCachedFlowSender;
use crate::protocol_runtime::vless_udp::VlessCachedFlowHandler;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessCachedFlowHandler;

pub(crate) struct CachedUdpHandlers {
    pub(crate) vless: Box<dyn VlessCachedFlowHandler>,
    #[cfg(feature = "vmess")]
    pub(crate) vmess: Box<dyn VmessCachedFlowHandler>,
}

pub(in crate::protocol_runtime::udp::state) struct CachedProtocolUdpState {
    handlers: CachedUdpHandlers,
}

impl CachedProtocolUdpState {
    pub(in crate::protocol_runtime::udp::state) fn new(handlers: CachedUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state) fn vless(
        &mut self,
    ) -> &mut dyn VlessCachedFlowHandler {
        self.handlers.vless.as_mut()
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp::state) fn vmess(
        &mut self,
    ) -> &mut dyn VmessCachedFlowHandler {
        self.handlers.vmess.as_mut()
    }

    pub(in crate::protocol_runtime::udp::state) fn senders(
        &mut self,
    ) -> impl Iterator<Item = &mut dyn ManagedCachedFlowSender> {
        let mut senders: Vec<&mut dyn ManagedCachedFlowSender> =
            vec![self.handlers.vless.as_mut() as &mut dyn ManagedCachedFlowSender];
        #[cfg(feature = "vmess")]
        senders.push(self.handlers.vmess.as_mut() as &mut dyn ManagedCachedFlowSender);
        senders.into_iter()
    }
}
