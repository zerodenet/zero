use std::collections::HashMap;

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

pub(super) struct ManagedStreamSenderState {
    senders: HashMap<ManagedUdpFlowRef, Box<dyn ManagedStreamFlowSender>>,
}

impl ManagedStreamSenderState {
    pub(super) fn new() -> Self {
        Self {
            senders: HashMap::new(),
        }
    }

    pub(super) fn sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<&mut dyn ManagedStreamFlowSender> {
        self.senders
            .get_mut(&flow_ref)
            .map(|handler| handler.as_mut() as &mut dyn ManagedStreamFlowSender)
    }

    pub(super) fn push_sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) {
        self.senders.insert(flow_ref, sender);
    }
}
