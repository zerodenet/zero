use tokio::task::JoinSet;
use zero_core::Address;

use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

#[async_trait::async_trait]
pub(crate) trait CachedProtocolFlowSender: Send + Sync {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, zero_engine::EngineError>;
}

pub(crate) struct CachedUdpHandlers {
    pub(crate) cached: Vec<Box<dyn CachedProtocolFlowSender>>,
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
    ) -> impl Iterator<Item = &mut dyn CachedProtocolFlowSender> {
        self.handlers
            .cached
            .iter_mut()
            .map(|handler| handler.as_mut() as &mut dyn CachedProtocolFlowSender)
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn push_sender(
        &mut self,
        sender: Box<dyn CachedProtocolFlowSender>,
    ) {
        self.handlers.cached.push(sender);
    }
}
