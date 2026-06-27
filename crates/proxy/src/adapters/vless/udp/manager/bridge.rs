use zero_core::Address;

use super::VlessUdpOutboundManager;
use crate::runtime::udp_flow::managed::ManagedStreamConnectionCacheKey;

impl VlessUdpOutboundManager {
    pub(super) fn spawn_bridge(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        target: Address,
        port: u16,
        session_id: u64,
    ) {
        let key = ManagedStreamConnectionCacheKey::new(target, port);
        if let Some(upstream) = self.upstreams.get(&key) {
            upstream
                .connection
                .spawn_response_bridge(chain_tasks, session_id);
        }
    }
}
