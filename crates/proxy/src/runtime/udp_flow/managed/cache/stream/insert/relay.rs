use zero_engine::EngineError;

use super::super::model::ManagedUdpConnectionCache;
use super::super::send::send_managed_udp_connection;
use crate::runtime::udp_flow::managed::connection::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

impl ManagedUdpConnectionCache {
    async fn insert_and_send(
        &mut self,
        key: super::super::super::key::ManagedUdpConnectionCacheKey,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
        connection: SharedManagedUdpConnection,
    ) -> Result<usize, EngineError> {
        let sent =
            send_managed_udp_connection(&connection, chain_tasks, session_id, packet).await?;
        self.entries.insert(key, connection);
        Ok(sent)
    }

    pub(crate) async fn insert_and_send_key(
        &mut self,
        key: impl Into<String>,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
        connection: SharedManagedUdpConnection,
    ) -> Result<usize, EngineError> {
        self.insert_and_send(
            super::super::super::key::ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            connection,
        )
        .await
    }
}
