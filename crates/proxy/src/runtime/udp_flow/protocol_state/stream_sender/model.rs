use tokio::task::JoinSet;
use zero_core::Address;

use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

#[async_trait::async_trait]
pub(crate) trait ManagedStreamFlowSender: Send + Sync {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, zero_engine::EngineError>;
}

pub(crate) struct ManagedStreamSenderHandlers {
    pub(crate) stream: Vec<Box<dyn ManagedStreamFlowSender>>,
}

pub(in crate::runtime::udp_flow::protocol_state) struct ManagedStreamSenderState {
    senders: std::collections::HashMap<ManagedUdpFlowRef, Box<dyn ManagedStreamFlowSender>>,
}

impl ManagedStreamSenderState {
    pub(in crate::runtime::udp_flow::protocol_state) fn new(
        mut handlers: ManagedStreamSenderHandlers,
    ) -> Self {
        debug_assert!(
            handlers.stream.is_empty(),
            "managed stream senders are registered after flow establishment"
        );
        handlers.stream.clear();
        Self {
            senders: std::collections::HashMap::new(),
        }
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<&mut dyn ManagedStreamFlowSender> {
        self.senders
            .get_mut(&flow_ref)
            .map(|handler| handler.as_mut() as &mut dyn ManagedStreamFlowSender)
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn contains_sender(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> bool {
        self.senders.contains_key(&flow_ref)
    }

    pub(in crate::runtime::udp_flow::protocol_state) fn push_sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) {
        self.senders.insert(flow_ref, sender);
    }
}
