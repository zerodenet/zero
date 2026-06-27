use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(in crate::protocol_runtime::udp::state) mod model;

pub(in crate::protocol_runtime::udp::state) use model::CachedProtocolUdpState;
pub(crate) use model::CachedUdpHandlers;

impl ProtocolUdpState {
    pub(in crate::protocol_runtime::udp) fn register_cached_flow_sender(
        &mut self,
        sender: Box<dyn crate::protocol_runtime::udp::ManagedCachedFlowSender>,
    ) {
        self.cached.push_sender(sender);
    }

    pub(crate) async fn send_existing_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        for sender in self.cached.senders() {
            if let Some(session_id) = sender
                .send_existing(chain_tasks, proxy, target, port, payload)
                .await?
            {
                return Ok(Some(session_id));
            }
        }

        Ok(None)
    }
}
