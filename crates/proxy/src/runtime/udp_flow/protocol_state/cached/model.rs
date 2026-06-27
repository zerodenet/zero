use tokio::task::JoinSet;
use zero_core::Address;

use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
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
    senders: std::collections::HashMap<ManagedUdpFlowRef, Box<dyn CachedProtocolFlowSender>>,
}

impl CachedProtocolUdpState {
    pub(in crate::runtime::udp_flow::protocol_state) fn new(
        mut handlers: CachedUdpHandlers,
    ) -> Self {
        debug_assert!(
            handlers.cached.is_empty(),
            "cached flow senders are registered after flow establishment"
        );
        handlers.cached.clear();
        Self {
            senders: std::collections::HashMap::new(),
        }
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<&mut dyn CachedProtocolFlowSender> {
        self.senders
            .get_mut(&flow_ref)
            .map(|handler| handler.as_mut() as &mut dyn CachedProtocolFlowSender)
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn push_sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
        sender: Box<dyn CachedProtocolFlowSender>,
    ) {
        self.senders.insert(flow_ref, sender);
    }
}
