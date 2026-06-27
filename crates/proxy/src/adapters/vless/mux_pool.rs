// VLESS MUX outbound connection pool.
//
// Reuses MUX connections to the same upstream (server + port + transport).
// Supports TCP, TLS, REALITY transports.
//
// Types moved to vless::mux_pool; this module handles
// connection establishment which depends on proxy I/O infrastructure.

mod model;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) use model::{MuxConnectionPool, VlessMuxOpenRequest};
use vless::mux_pool::{
    open_mux_tcp_stream, open_mux_udp_stream, MuxPoolConn, PoolKey, TransportKey,
};

impl std::fmt::Debug for MuxConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MuxConnectionPool")
            .field("entries", &self.pool.lock().unwrap().len())
            .finish()
    }
}

impl MuxConnectionPool {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Drop all cached connections.  Call after a config reload that may
    /// have changed upstream server addresses or credentials.
    pub fn evict_all(&self) {
        let mut guard = self.pool.lock().expect("mux pool lock poisoned");
        guard.clear();
    }

    /// Look up or create a MUX connection to the given upstream.
    async fn get_or_create_conn(
        &self,
        request: &VlessMuxOpenRequest<'_>,
    ) -> Result<Arc<MuxPoolConn>, EngineError> {
        let transport = match (request.tls, request.reality) {
            (Some(t), None) => TransportKey::Tls {
                server_name: t.server_name.clone(),
            },
            (None, Some(r)) => TransportKey::Reality {
                public_key: r.public_key.clone(),
                server_name: r
                    .server_name
                    .clone()
                    .unwrap_or_else(|| request.server.to_owned()),
            },
            _ => TransportKey::Raw,
        };

        let key = PoolKey::from_identity(
            request.server.to_owned(),
            request.port,
            request.identity.clone(),
            transport,
        );

        let conn = {
            let pool = self.pool.lock().unwrap();
            pool.get(&key).cloned()
        };

        match conn {
            Some(c) => {
                if *c.active.lock().unwrap() >= c.max_concurrency as usize {
                    let conn = Self::create_mux_connection(request.proxy, &key, request).await?;
                    let conn = Arc::new(conn);
                    self.pool.lock().unwrap().insert(key, conn.clone());
                    Ok(conn)
                } else {
                    Ok(c)
                }
            }
            None => {
                let conn = Self::create_mux_connection(request.proxy, &key, request).await?;
                let conn = Arc::new(conn);
                self.pool.lock().unwrap().insert(key, conn.clone());
                Ok(conn)
            }
        }
    }

    pub async fn open_stream(
        &self,
        request: VlessMuxOpenRequest<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        self.open_stream_inner(request, vless::NETWORK_TCP).await
    }

    /// Open a UDP MUX sub-stream (SIP022 Mux.Cool NETWORK_UDP).
    ///
    /// Per Xray-core Mux.Cool semantics: UDP MUX sub-streams are
    /// connectionless -?each STATUS_KEEP frame carries its own
    /// `[network:1][port:2][atyp:1][address…]` prefix, so one UDP
    /// sub-stream can serve all targets through this MUX connection.
    ///
    /// Returns `(session_id, write_tx, down_rx)` -?a tuple of:
    ///   - `session_id` -?MUX session id for response dispatch
    ///   - `write_tx` -?unbounded sender, submit raw VLESS UDP packets
    ///   - `down_rx` -?unbounded receiver for responses
    pub async fn open_udp_stream(
        &self,
        request: VlessMuxOpenRequest<'_>,
    ) -> Result<
        (
            u16,
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ),
        EngineError,
    > {
        let conn = self.get_or_create_conn(&request).await?;

        let stream = open_mux_udp_stream(conn)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
        Ok((stream.session_id, stream.up_tx, stream.down_rx))
    }

    async fn open_stream_inner(
        &self,
        request: VlessMuxOpenRequest<'_>,
        network: u8,
    ) -> Result<TcpRelayStream, EngineError> {
        let session = request
            .session
            .expect("VLESS TCP MUX stream requires session target");
        let _ = network; // network is used by future callers (e.g. open_udp_stream), TcpStream path hardcodes NETWORK_TCP
        let conn = self.get_or_create_conn(&request).await?;

        let stream = open_mux_tcp_stream(conn, session.port, &session.target)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        Ok(TcpRelayStream::new(stream))
    }

    async fn create_mux_connection(
        proxy: &Proxy,
        key: &PoolKey,
        request: &VlessMuxOpenRequest<'_>,
    ) -> Result<MuxPoolConn, EngineError> {
        use crate::transport::MeteredStream;

        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(&key.server, key.port, proxy.resolver.as_ref())
            .await?;

        let connector = crate::transport::VlessTransportConnector::new(
            crate::transport::VlessTransportOptions {
                tls: request.tls,
                reality: request.reality,
                ws: None,
                grpc: None,
                h2: None,
                http_upgrade: None,
                split_http: None,
                source_dir: proxy.config.source_dir(),
            },
        );
        let stream: TcpRelayStream = connector.connect(socket, &key.server, key.port).await?;

        let mut metered = MeteredStream::new(stream);
        let _mux = vless::VlessOutbound
            .establish_mux(&mut metered, key.uuid())
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        Ok(MuxPoolConn::new(
            metered.into_inner(),
            key.uuid(),
            request.max_concurrency,
        ))
    }
}
