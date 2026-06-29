mod model;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zero_core::Network;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::transport::{MeteredStream, TcpRelayStream, VmessTransportConnector};

pub(crate) use model::{VmessMuxConnectionPool, VmessMuxOpenRequest};

impl std::fmt::Debug for VmessMuxConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmessMuxConnectionPool")
            .field("entries", &self.pool.lock().unwrap().len())
            .finish()
    }
}

impl VmessMuxConnectionPool {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn evict_all(&self) {
        self.pool.lock().expect("vmess mux pool poisoned").clear();
    }

    pub async fn open_stream(
        &self,
        request: VmessMuxOpenRequest<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        self.open_with_network(request, Network::Tcp).await
    }

    pub async fn open_udp_stream(
        &self,
        request: VmessMuxOpenRequest<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        self.open_with_network(request, Network::Udp).await
    }

    async fn open_with_network(
        &self,
        request: VmessMuxOpenRequest<'_>,
        network: Network,
    ) -> Result<TcpRelayStream, EngineError> {
        let key = request.pool_key().map_err(|error| {
            EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
        })?;

        let conn = self.get_or_create_conn(&key, &request).await?;
        Ok(TcpRelayStream::new(conn.open_stream(
            request.session.target.clone(),
            request.session.port,
            network,
        )))
    }

    async fn get_or_create_conn(
        &self,
        key: &vmess::mux::VmessMuxPoolKey,
        request: &VmessMuxOpenRequest<'_>,
    ) -> Result<Arc<vmess::mux::VmessMuxConn>, EngineError> {
        let cached = self.pool.lock().unwrap().get(key).cloned();
        let conn = match cached {
            Some(conn) if conn.has_capacity() => conn,
            _ => {
                let conn = Arc::new(Self::create_connection(key, request).await?);
                self.pool.lock().unwrap().insert(key.clone(), conn.clone());
                conn
            }
        };
        Ok(conn)
    }

    async fn create_connection(
        key: &vmess::mux::VmessMuxPoolKey,
        request: &VmessMuxOpenRequest<'_>,
    ) -> Result<vmess::mux::VmessMuxConn, EngineError> {
        let socket = request
            .proxy
            .protocols
            .direct_connector()
            .connect_host(&key.server, key.port, request.proxy.resolver.as_ref())
            .await?;

        let connector = VmessTransportConnector::new(crate::transport::VmessTransportOptions {
            tls: request.tls,
            ws: request.ws,
            grpc: request.grpc,
            source_dir: request.proxy.config.source_dir(),
        });
        let stream = connector.connect(socket, &key.server, key.port).await?;

        let metered = MeteredStream::new(stream);
        let stream = TcpRelayStream::new(key.establish_mux_outbound_stream(metered).await?);

        Ok(key.clone().into_pool_conn(stream, request.max_concurrency))
    }
}
