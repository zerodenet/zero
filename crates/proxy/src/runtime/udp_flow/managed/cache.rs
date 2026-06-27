use std::collections::HashMap;
use std::future::Future;

use zero_core::Address;
use zero_engine::EngineError;

use super::SharedManagedDatagramUdpConnection;
use super::SharedManagedUdpConnection;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpPacketRef};

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

    pub(crate) fn get(
        &self,
        key: &ManagedUdpConnectionCacheKey,
    ) -> Option<&SharedManagedUdpConnection> {
        self.entries.get(key)
    }

    pub(crate) fn insert(
        &mut self,
        key: ManagedUdpConnectionCacheKey,
        value: SharedManagedUdpConnection,
    ) -> Option<SharedManagedUdpConnection> {
        self.entries.insert(key, value)
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

pub(crate) struct ManagedStreamConnectionCache {
    entries: HashMap<ManagedStreamConnectionCacheKey, ManagedStreamConnection>,
}

impl ManagedStreamConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn get(
        &self,
        key: &ManagedStreamConnectionCacheKey,
    ) -> Option<&ManagedStreamConnection> {
        self.entries.get(key)
    }

    pub(crate) fn insert(
        &mut self,
        key: ManagedStreamConnectionCacheKey,
        value: ManagedStreamConnection,
    ) -> Option<ManagedStreamConnection> {
        self.entries.insert(key, value)
    }
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

    pub(crate) fn get(
        &self,
        key: &ManagedDatagramConnectionCacheKey,
    ) -> Option<&SharedManagedDatagramUdpConnection> {
        self.entries.get(key)
    }

    pub(crate) fn insert(
        &mut self,
        key: ManagedDatagramConnectionCacheKey,
        value: SharedManagedDatagramUdpConnection,
    ) -> Option<SharedManagedDatagramUdpConnection> {
        self.entries.insert(key, value)
    }
}
