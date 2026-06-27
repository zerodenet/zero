use zero_core::Address;
use zero_engine::EngineError;

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
            let mut recv_rx = upstream.connection.subscribe_responses();
            chain_tasks.spawn(async move {
                let packet = recv_rx
                    .recv()
                    .await
                    .map_err(|_| EngineError::Io(std::io::Error::other("vless upstream closed")))?;
                Ok((packet.0, packet.1, packet.2, Some(session_id)))
            });
        }
    }
}
