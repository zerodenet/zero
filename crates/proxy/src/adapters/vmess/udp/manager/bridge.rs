use zero_core::Address;

use super::VmessUdpOutboundManager;
use crate::runtime::udp_flow::managed::spawn_tuple_response_bridge;

impl VmessUdpOutboundManager {
    pub(super) fn spawn_bridge(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        target: Address,
        port: u16,
        session_id: u64,
    ) {
        if let Some(upstream) = self.upstreams.get(&(target.clone(), port)) {
            spawn_tuple_response_bridge(
                chain_tasks,
                upstream.connection.subscribe_responses(),
                session_id,
                "vmess upstream closed",
            );
        }
    }
}
