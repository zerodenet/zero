use std::future::Future;

use zero_engine::EngineError;

use super::super::model::ManagedUdpConnectionCache;
use super::super::send::send_managed_udp_connection;
use crate::runtime::udp_flow::managed::connection::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

impl ManagedUdpConnectionCache {
    async fn send_or_insert<Fut>(
        &mut self,
        key: super::super::super::key::ManagedUdpConnectionCacheKey,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
        establish: Fut,
    ) -> Result<usize, EngineError>
    where
        Fut: Future<Output = Result<SharedManagedUdpConnection, EngineError>>,
    {
        if let Some(connection) = self.entries.get(&key) {
            return send_managed_udp_connection(connection, chain_tasks, session_id, packet).await;
        }

        let connection = establish.await?;
        let sent =
            send_managed_udp_connection(&connection, chain_tasks, session_id, packet).await?;
        self.entries.insert(key, connection);
        Ok(sent)
    }

    pub(crate) async fn send_or_insert_key<Fut>(
        &mut self,
        key: impl Into<String>,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
        establish: Fut,
    ) -> Result<usize, EngineError>
    where
        Fut: Future<Output = Result<SharedManagedUdpConnection, EngineError>>,
    {
        self.send_or_insert(
            super::super::super::key::ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            establish,
        )
        .await
    }
}
