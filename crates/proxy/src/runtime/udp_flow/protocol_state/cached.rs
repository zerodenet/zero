use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(super) mod model;

pub(crate) use model::CachedProtocolFlowSender;
pub(super) use model::CachedProtocolUdpState;
pub(crate) use model::CachedUdpHandlers;

impl ProtocolUdpState {
    pub(crate) fn register_cached_flow_sender(
        &mut self,
        sender: Box<dyn CachedProtocolFlowSender>,
    ) -> ManagedUdpFlowRef {
        let flow_ref = self.next_managed_flow_ref();
        self.cached.push_sender(flow_ref, sender);
        flow_ref
    }

    pub(crate) async fn send_existing_cached_flow(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let Some(sender) = self.cached.sender(flow_ref) else {
            return Ok(None);
        };
        sender
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await
    }
}
