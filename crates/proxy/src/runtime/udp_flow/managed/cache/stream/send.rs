use zero_engine::EngineError;

use super::model::ManagedUdpConnectionCache;
use crate::runtime::udp_flow::managed::connection::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

pub(super) async fn send_managed_udp_connection(
    connection: &SharedManagedUdpConnection,
    chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
    session_id: u64,
    packet: UdpPacketRef<'_>,
) -> Result<usize, EngineError> {
    connection.spawn_response_bridge(chain_tasks, session_id);
    connection
        .send(packet.target, packet.port, packet.payload)
        .await
}

impl ManagedUdpConnectionCache {
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) async fn send_existing_key(
        &self,
        key: impl Into<String>,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
    ) -> Result<Option<usize>, EngineError> {
        let key = super::super::key::ManagedUdpConnectionCacheKey::new(key);
        let Some(connection) = self.entries.get(&key) else {
            return Ok(None);
        };
        send_managed_udp_connection(connection, chain_tasks, session_id, packet)
            .await
            .map(Some)
    }
}
