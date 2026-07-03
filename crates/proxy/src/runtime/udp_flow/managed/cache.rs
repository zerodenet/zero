use std::collections::HashMap;
use std::future::Future;

use zero_engine::EngineError;

use super::connection::{SharedManagedDatagramUdpConnection, SharedManagedUdpConnection};
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ManagedUdpConnectionCacheKey(String);

impl ManagedUdpConnectionCacheKey {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub(crate) struct ManagedUdpConnectionCache {
    entries: HashMap<ManagedUdpConnectionCacheKey, SharedManagedUdpConnection>,
}

impl ManagedUdpConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    async fn send_or_insert_pre_sent<Fut>(
        &mut self,
        key: ManagedUdpConnectionCacheKey,
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
            ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            establish,
        )
        .await
    }

    async fn send_or_insert<Fut>(
        &mut self,
        key: ManagedUdpConnectionCacheKey,
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
            ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            establish,
        )
        .await
    }

    async fn insert_and_send(
        &mut self,
        key: ManagedUdpConnectionCacheKey,
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
            ManagedUdpConnectionCacheKey::new(key),
            chain_tasks,
            session_id,
            packet,
            connection,
        )
        .await
    }

    pub(crate) async fn send_existing_key(
        &self,
        key: impl Into<String>,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
        packet: UdpPacketRef<'_>,
    ) -> Result<Option<usize>, EngineError> {
        let key = ManagedUdpConnectionCacheKey::new(key);
        let Some(connection) = self.entries.get(&key) else {
            return Ok(None);
        };
        send_managed_udp_connection(connection, chain_tasks, session_id, packet)
            .await
            .map(Some)
    }
}

async fn send_managed_udp_connection(
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ManagedDatagramConnectionCacheKey(String);

impl ManagedDatagramConnectionCacheKey {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub(crate) struct ManagedDatagramConnectionCache {
    entries: HashMap<ManagedDatagramConnectionCacheKey, SharedManagedDatagramUdpConnection>,
}

impl ManagedDatagramConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    async fn get_or_insert_with<Fut>(
        &mut self,
        key: ManagedDatagramConnectionCacheKey,
        establish: Fut,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError>
    where
        Fut: Future<Output = Result<SharedManagedDatagramUdpConnection, EngineError>>,
    {
        if let Some(connection) = self.entries.get(&key) {
            return Ok(connection.clone());
        }

        let connection = establish.await?;
        self.entries.insert(key, connection.clone());
        Ok(connection)
    }

    pub(crate) async fn get_or_insert_key<Fut>(
        &mut self,
        key: impl Into<String>,
        establish: Fut,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError>
    where
        Fut: Future<Output = Result<SharedManagedDatagramUdpConnection, EngineError>>,
    {
        self.get_or_insert_with(ManagedDatagramConnectionCacheKey::new(key), establish)
            .await
    }
}
