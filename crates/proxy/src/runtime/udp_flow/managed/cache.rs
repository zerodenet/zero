use std::collections::HashMap;
use std::future::Future;

use zero_core::Address;
use zero_engine::EngineError;

use super::SharedManagedDatagramUdpConnection;
use super::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};
use crate::runtime::Proxy;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ManagedUdpConnectionCacheKey(String);

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

    pub(crate) async fn send_or_insert_pre_sent<Fut>(
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

    pub(crate) async fn send_or_insert<Fut>(
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

    pub(crate) async fn insert_and_send(
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
pub(crate) struct ManagedStreamConnectionCacheKey {
    target: Address,
    port: u16,
}

impl ManagedStreamConnectionCacheKey {
    pub(crate) fn new(target: Address, port: u16) -> Self {
        Self { target, port }
    }
}

pub(crate) struct ManagedStreamConnection {
    pub(crate) session_id: u64,
    pub(crate) connection: SharedManagedUdpConnection,
}

impl ManagedStreamConnection {
    pub(crate) fn new(session_id: u64, connection: SharedManagedUdpConnection) -> Self {
        Self {
            session_id,
            connection,
        }
    }
}

pub(crate) struct ManagedStreamConnectionSend<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<ChainTask>,
    pub(crate) proxy: &'a Proxy,
    pub(crate) target: &'a Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedStreamConnectionCache {
    entries: HashMap<ManagedStreamConnectionCacheKey, ManagedStreamConnection>,
}

impl ManagedStreamConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn insert_and_bridge(
        &mut self,
        key: ManagedStreamConnectionCacheKey,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        upstream: ManagedStreamConnection,
    ) -> Option<ManagedStreamConnection> {
        let session_id = upstream.session_id;
        upstream
            .connection
            .spawn_response_bridge(chain_tasks, session_id);
        self.entries.insert(key, upstream)
    }

    pub(crate) fn insert_and_bridge_target(
        &mut self,
        target: Address,
        port: u16,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        upstream: ManagedStreamConnection,
    ) -> Option<ManagedStreamConnection> {
        self.insert_and_bridge(
            ManagedStreamConnectionCacheKey::new(target, port),
            chain_tasks,
            upstream,
        )
    }

    pub(crate) async fn send_existing(
        &self,
        key: ManagedStreamConnectionCacheKey,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let Some(upstream) = self.entries.get(&key) else {
            return Ok(None);
        };

        send_stream_connection(upstream, chain_tasks, proxy, target, port, payload).await?;
        Ok(Some(upstream.session_id))
    }

    pub(crate) async fn send_existing_target(
        &self,
        target: &Address,
        port: u16,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        proxy: &Proxy,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.send_existing(
            ManagedStreamConnectionCacheKey::new(target.clone(), port),
            chain_tasks,
            proxy,
            target,
            port,
            payload,
        )
        .await
    }

    pub(crate) async fn send_or_insert<Fut>(
        &mut self,
        key: ManagedStreamConnectionCacheKey,
        request: ManagedStreamConnectionSend<'_>,
        establish: Fut,
    ) -> Result<(), EngineError>
    where
        Fut: Future<Output = Result<ManagedStreamConnection, EngineError>>,
    {
        if let Some(upstream) = self.entries.get(&key) {
            send_stream_connection(
                upstream,
                request.chain_tasks,
                request.proxy,
                request.target,
                request.port,
                request.payload,
            )
            .await?;
            return Ok(());
        }

        let upstream = establish.await?;
        self.insert_and_bridge(key, request.chain_tasks, upstream);
        Ok(())
    }

    pub(crate) async fn send_or_insert_target<Fut>(
        &mut self,
        target: &Address,
        port: u16,
        request: ManagedStreamConnectionSend<'_>,
        establish: Fut,
    ) -> Result<(), EngineError>
    where
        Fut: Future<Output = Result<ManagedStreamConnection, EngineError>>,
    {
        self.send_or_insert(
            ManagedStreamConnectionCacheKey::new(target.clone(), port),
            request,
            establish,
        )
        .await
    }
}

async fn send_stream_connection(
    upstream: &ManagedStreamConnection,
    chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
    proxy: &Proxy,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<(), EngineError> {
    proxy.record_session_inbound_rx(upstream.session_id, payload.len() as u64);
    let packet_len = upstream.connection.send(target, port, payload).await? as u64;
    proxy.record_session_outbound_tx(upstream.session_id, packet_len);
    upstream
        .connection
        .spawn_response_bridge(chain_tasks, upstream.session_id);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ManagedDatagramConnectionCacheKey(String);

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

    pub(crate) async fn get_or_insert_with<Fut>(
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
}
