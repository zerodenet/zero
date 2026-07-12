use std::future::Future;

use zero_engine::EngineError;

use super::model::ManagedUdpConnectionCache;
use super::send::send_managed_udp_connection;
use crate::runtime::udp_flow::managed::connection::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

impl ManagedUdpConnectionCache {
    async fn send_or_insert_pre_sent<Fut>(
        &mut self,
        key: super::super::key::ManagedUdpConnectionCacheKey,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
        establish: Fut,
    ) -> Result<usize, EngineError>
    where
        Fut: Future<Output = Result<SharedManagedUdpConnection, EngineError>>,
    {
        let sent = packet.payload.len();
        if let Some(connection) = self.entries.get(&key) {
            connection.spawn_response_bridge(chain_tasks, session_id);
            return connection
                .send(packet.target, packet.port, packet.payload)
                .await;
        }

        let connection = establish.await?;
        connection.spawn_response_bridge(chain_tasks, session_id);
        self.entries.insert(key, connection);
        Ok(sent)
    }

    pub(crate) async fn send_or_insert_pre_sent_key<Fut>(
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
        self.send_or_insert_pre_sent(
            super::super::key::ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            establish,
        )
        .await
    }

    async fn send_or_insert<Fut>(
        &mut self,
        key: super::super::key::ManagedUdpConnectionCacheKey,
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
            super::super::key::ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            establish,
        )
        .await
    }

    async fn insert_and_send(
        &mut self,
        key: super::super::key::ManagedUdpConnectionCacheKey,
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
            super::super::key::ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            connection,
        )
        .await
    }
}
