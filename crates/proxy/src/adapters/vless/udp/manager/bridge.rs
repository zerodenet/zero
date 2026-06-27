use zero_core::Address;

use super::VlessUdpOutboundManager;

impl VlessUdpOutboundManager {
    pub(super) fn spawn_bridge(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        target: Address,
        port: u16,
        session_id: u64,
    ) {
        if let Some(upstream) = self.upstreams.get(&(target.clone(), port)) {
            upstream
                .connection
                .spawn_response_bridge(chain_tasks, session_id);
        }
    }
}
