use std::collections::HashMap;
use std::future::Future;

use zero_engine::EngineError;

use super::key::ManagedDatagramConnectionCacheKey;
use crate::runtime::udp_flow::managed::connection::SharedManagedDatagramUdpConnection;

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
