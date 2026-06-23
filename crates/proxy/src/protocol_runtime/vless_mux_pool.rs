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
    decrypt_mux_payload, encrypt_mux_payload, MuxPoolConn, MuxStreamRelay, PoolKey, TransportKey,
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

        let key = PoolKey {
            server: request.server.to_owned(),
            port: request.port,
            uuid: *request.id,
            transport,
        };

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
    /// connectionless — each STATUS_KEEP frame carries its own
    /// `[network:1][port:2][atyp:1][address…]` prefix, so one UDP
    /// sub-stream can serve all targets through this MUX connection.
    ///
    /// Returns `(session_id, write_tx, down_rx)` — a tuple of:
    ///   - `session_id` — MUX session id for response dispatch
    ///   - `write_tx` — unbounded sender, submit raw VLESS UDP packets
    ///   - `down_rx` — unbounded receiver for responses
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

        let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        conn.streams.lock().unwrap().insert(sid, down_tx);

        // Send new-stream request with NETWORK_UDP
        let req = vless::encode_new_stream(
            vless::NETWORK_UDP,
            0,                                       /* port unused for UDP MUX sub-stream */
            &zero_core::Address::Ipv4([0, 0, 0, 0]), /* address unused */
        )
        .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
        conn.write_tx
            .send(req)
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        // Upload relay: raw VLESS UDP packets → encrypt → MUX frame → write_tx
        let write = conn.write_tx.clone();
        let conn_drop = conn.clone();
        let crypto = conn.crypto.clone();
        tokio::spawn(async move {
            let mut up_rx = up_rx;
            while let Some(vless_udp_packet) = up_rx.recv().await {
                let payload = encrypt_mux_payload(&crypto, sid, &vless_udp_packet, true);
                // UDP MUX data frames: the VLESS UDP packet is the full payload
                let frame = vless::encode_data_frame(sid, &payload);
                if write.send(frame).is_err() {
                    break;
                }
            }
            let close_frame = vless::encode_end_frame(sid);
            let _ = write.send(close_frame);
            *conn_drop.active.lock().unwrap() -= 1;
        });

        Ok((sid, up_tx, down_rx))
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
        let req = vless::encode_new_stream(vless::NETWORK_TCP, session.port, &session.target)
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
                let frame = vless::encode_data_frame(sid, &payload);
                if write.send(frame).is_err() {
                    break;
                }
            }
            let close_frame = vless::encode_end_frame(sid);
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
        let _mux = proxy
            .protocols
            .vless_outbound_protocol()
            .establish_mux(&mut metered, &key.uuid)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;

        let tcp: TcpRelayStream = metered.into_inner();
        let (tcp_read, tcp_write) = tokio::io::split(tcp);

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let crypto: Option<Arc<Mutex<vless::MuxCrypto>>> =
            Some(Arc::new(Mutex::new(vless::MuxCrypto::new(&key.uuid))));

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
            max_concurrency: request.max_concurrency,
            crypto,
        })
    }
}
