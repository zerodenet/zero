// VLESS MUX outbound connection pool.
//
// Reuses MUX connections to the same upstream (server + port + transport).
// Supports TCP, TLS, REALITY transports.
//
// Types moved to zero_protocol_vless::mux_pool; this module handles
// connection establishment which depends on proxy I/O infrastructure.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

use zero_config::{ClientTlsConfig, RealityConfig};
use zero_protocol_vless::mux_pool::{
    decrypt_mux_payload, encrypt_mux_payload, MuxPoolConn, MuxStreamRelay, PoolKey, TransportKey,
};

#[derive(Clone)]
pub(crate) struct MuxConnectionPool {
    pool: Arc<Mutex<HashMap<PoolKey, Arc<MuxPoolConn>>>>,
}

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

    pub async fn open_stream(
        &self,
        proxy: &Proxy,
        session: &Session,
        server: String,
        port: u16,
        id: &[u8; 16],
        tls: Option<&ClientTlsConfig>,
        reality: Option<&RealityConfig>,
        max_concurrency: u32,
        _idle_timeout_secs: u64,
    ) -> Result<TcpRelayStream, EngineError> {
        let transport = match (tls, reality) {
            (Some(t), None) => TransportKey::Tls {
                server_name: t.server_name.clone(),
            },
            (None, Some(r)) => TransportKey::Reality {
                public_key: r.public_key.clone(),
                server_name: r.server_name.clone().unwrap_or(server.clone()),
            },
            _ => TransportKey::Raw,
        };

        let key = PoolKey {
            server,
            port,
            uuid: *id,
            transport,
        };

        let conn = {
            let pool = self.pool.lock().unwrap();
            pool.get(&key).cloned()
        };

        let conn = match conn {
            Some(c) => {
                if *c.active.lock().unwrap() >= c.max_concurrency as usize {
                    let conn =
                        Self::create_mux_connection(proxy, &key, tls, reality, max_concurrency)
                            .await?;
                    let conn = Arc::new(conn);
                    self.pool.lock().unwrap().insert(key, conn.clone());
                    conn
                } else {
                    c
                }
            }
            None => {
                let conn =
                    Self::create_mux_connection(proxy, &key, tls, reality, max_concurrency).await?;
                let conn = Arc::new(conn);
                self.pool.lock().unwrap().insert(key, conn.clone());
                conn
            }
        };

        // Allocate stream ID
        let sid = {
            let mut next = conn.next_id.lock().unwrap();
            let s = *next;
            *next = next.wrapping_add(1);
            if *next == 0 {
                *next = 1;
            }
            s
        };

        *conn.active.lock().unwrap() += 1;

        let (_up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        conn.streams.lock().unwrap().insert(sid, down_tx);

        // Send new-stream request to the peer
        let req = zero_protocol_vless::encode_new_stream(session.port, &session.target)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
        conn.write_tx
            .send(req)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        // Spawn upload relay: up_rx → encrypt → MUX frame → write_tx
        let write = conn.write_tx.clone();
        let conn_drop = conn.clone();
        let crypto = conn.crypto.clone();
        tokio::spawn(async move {
            let mut up_rx = up_rx;
            while let Some(data) = up_rx.recv().await {
                let payload = encrypt_mux_payload(&crypto, sid, &data, true);
                let frame = zero_protocol_vless::encode_frame(sid, &payload);
                if write.send(frame).is_err() {
                    break;
                }
            }
            let close_frame = zero_protocol_vless::encode_frame(sid, &[]);
            let _ = write.send(close_frame);
            *conn_drop.active.lock().unwrap() -= 1;
        });

        let stream = MuxStreamRelay {
            up_tx: conn.write_tx.clone(),
            sid,
            down_rx: Some(down_rx),
            conn: conn.clone(),
        };

        Ok(TcpRelayStream::new(stream))
    }

    async fn create_mux_connection(
        proxy: &Proxy,
        key: &PoolKey,
        tls: Option<&ClientTlsConfig>,
        reality: Option<&RealityConfig>,
        max_concurrency: u32,
    ) -> Result<MuxPoolConn, EngineError> {
        use crate::transport::MeteredStream;

        let socket = proxy
            .protocols
            .direct_outbound
            .connect_host(&key.server, key.port, proxy.resolver.as_ref())
            .await?;

        let connector = crate::transport::VlessTransportConnector::new(
            tls,
            reality,
            None,
            None,
            None,
            None,
            None,
            proxy.config.source_dir(),
        );
        let stream: TcpRelayStream = connector
            .connect(socket, &key.server, key.port)
            .await?;

        let mut metered = MeteredStream::new(stream);
        let _mux = proxy
            .protocols
            .vless_outbound
            .establish_mux(&mut metered, &key.uuid)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        let tcp: TcpRelayStream = metered.into_inner().into();
        let (tcp_read, tcp_write) = tokio::io::split(tcp);

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let crypto: Option<Arc<Mutex<zero_protocol_vless::MuxCrypto>>> =
            Some(Arc::new(Mutex::new(zero_protocol_vless::MuxCrypto::new(
                &key.uuid,
            ))));

        // Write relay: frames → TCP
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut w = tcp_write;
            while let Some(frame) = write_rx.recv().await {
                if w.write_all(&frame).await.is_err() {
                    break;
                }
            }
            let _ = w.shutdown().await;
        });

        let streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let streams_for_relay = streams.clone();
        let streams_for_pool = streams;

        // Read relay: TCP → dispatch MUX frames → decrypt → stream channels
        let crypto_for_read = crypto.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut r = tcp_read;
            let mut buf = [0u8; 4];
            loop {
                if r.read_exact(&mut buf).await.is_err() {
                    break;
                }
                let stream_id = u16::from_be_bytes([buf[0], buf[1]]);
                let length = u16::from_be_bytes([buf[2], buf[3]]) as usize;
                if length > 16384 {
                    break;
                }
                let mut payload = vec![0u8; length];
                if length > 0 && r.read_exact(&mut payload).await.is_err() {
                    break;
                }

                if stream_id != 0 {
                    let decrypted =
                        decrypt_mux_payload(&crypto_for_read, stream_id, &payload, false);
                    if let Some(decrypted_payload) = decrypted {
                        let streams = streams_for_relay.lock().unwrap();
                        if let Some(tx) = streams.get(&stream_id) {
                            let _ = tx.send(decrypted_payload);
                        }
                    }
                }
            }
        });

        Ok(MuxPoolConn {
            write_tx,
            streams: streams_for_pool,
            next_id: Mutex::new(1),
            active: Mutex::new(0),
            max_concurrency,
            crypto,
        })
    }
}
