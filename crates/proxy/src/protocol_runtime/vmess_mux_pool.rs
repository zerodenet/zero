pub(crate) mod model;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::Network;
use zero_engine::EngineError;

use crate::transport::{MeteredStream, TcpRelayStream};

pub(crate) use model::VmessMuxConnectionPool;
use model::{VmessMuxConn, VmessMuxOpenRequest, VmessMuxPoolKey, VmessMuxTransportKey};

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
        let key = VmessMuxPoolKey {
            server: request.server.clone(),
            port: request.port,
            id: request.id,
            cipher_name: request.cipher_name.clone(),
            transport: transport_key(request.tls, request.ws, request.grpc)?,
        };

        let conn = self.get_or_create_conn(&key, &request).await?;
        let session_id = {
            let mut next = conn.next_id.lock().unwrap();
            let id = *next;
            *next = next.wrapping_add(1);
            if *next == 0 {
                *next = 1;
            }
            id
        };

        *conn.active.lock().unwrap() += 1;
        let (down_tx, down_rx) = mpsc::unbounded_channel();
        conn.streams.lock().unwrap().insert(session_id, down_tx);

        Ok(TcpRelayStream::new(vmess::mux_stream_with_network(
            session_id,
            request.session.target.clone(),
            request.session.port,
            network,
            conn.write_tx.clone(),
            down_rx,
            conn.active.clone(),
        )))
    }

    async fn get_or_create_conn(
        &self,
        key: &VmessMuxPoolKey,
        request: &VmessMuxOpenRequest<'_>,
    ) -> Result<Arc<VmessMuxConn>, EngineError> {
        let cached = self.pool.lock().unwrap().get(key).cloned();
        let conn = match cached {
            Some(conn) if *conn.active.lock().unwrap() < conn.max_concurrency as usize => conn,
            _ => {
                let conn = Arc::new(Self::create_connection(key, request).await?);
                self.pool.lock().unwrap().insert(key.clone(), conn.clone());
                conn
            }
        };
        Ok(conn)
    }

    async fn create_connection(
        key: &VmessMuxPoolKey,
        request: &VmessMuxOpenRequest<'_>,
    ) -> Result<VmessMuxConn, EngineError> {
        let socket = request
            .proxy
            .protocols
            .direct_connector()
            .connect_host(&key.server, key.port, request.proxy.resolver.as_ref())
            .await?;

        let stream = connect_vmess_transport(
            socket,
            request.tls,
            request.ws,
            request.grpc,
            request.proxy.config.source_dir(),
            &key.server,
            key.port,
        )
        .await?;

        let mut metered = MeteredStream::new(stream);
        let mux_target = vmess::mux_cool_session();
        let mux_session = vmess::VmessOutbound
            .establish_tcp_session(&mut metered, &mux_target, &key.id, request.cipher)
            .await?;
        let stream = TcpRelayStream::new(vmess::VmessAeadStream::outbound(
            metered.into_inner(),
            mux_session,
        )?);

        let (mut reader, mut writer) = tokio::io::split(stream);
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        tokio::spawn(async move {
            while let Some(frame) = write_rx.recv().await {
                if writer.write_all(&frame).await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
            }
            let _ = writer.shutdown().await;
        });

        let streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let streams_for_read = streams.clone();

        tokio::spawn(async move {
            loop {
                let frame = match vmess::read_mux_frame_from_tokio(&mut reader).await {
                    Ok(frame) => frame,
                    Err(_) => break,
                };
                if frame.status == vmess::MUX_STATUS_KEEP_ALIVE {
                    continue;
                }
                let tx = streams_for_read
                    .lock()
                    .unwrap()
                    .get(&frame.session_id)
                    .cloned();
                if let Some(tx) = tx {
                    if frame.status == vmess::MUX_STATUS_END {
                        let _ = tx.send(Vec::new());
                        streams_for_read.lock().unwrap().remove(&frame.session_id);
                    } else if !frame.payload.is_empty() {
                        let _ = tx.send(frame.payload);
                    }
                }
            }
        });

        Ok(VmessMuxConn {
            write_tx,
            streams,
            next_id: Mutex::new(1),
            active: Arc::new(Mutex::new(0)),
            max_concurrency: request.max_concurrency,
        })
    }
}

async fn connect_vmess_transport(
    socket: zero_platform_tokio::TokioSocket,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
    source_dir: Option<&std::path::Path>,
    server: &str,
    port: u16,
) -> Result<TcpRelayStream, EngineError> {
    match (grpc, ws, tls) {
        (Some(grpc_cfg), None, Some(tls_cfg)) => {
            let tls_stream =
                zero_transport::tls::connect_tls_upstream(socket, tls_cfg, source_dir, server)
                    .await?;
            Ok(TcpRelayStream::new(
                zero_transport::grpc::connect_grpc(tls_stream, &grpc_cfg.service_names).await?,
            ))
        }
        (Some(grpc_cfg), None, None) => Ok(TcpRelayStream::new(
            zero_transport::grpc::connect_grpc(socket, &grpc_cfg.service_names).await?,
        )),
        (None, Some(ws_cfg), Some(tls_cfg)) => {
            let tls_stream =
                zero_transport::tls::connect_tls_upstream(socket, tls_cfg, source_dir, server)
                    .await?;
            Ok(TcpRelayStream::new(
                zero_transport::ws::connect_ws(tls_stream, ws_cfg, server, port).await?,
            ))
        }
        (None, Some(ws_cfg), None) => Ok(TcpRelayStream::new(
            zero_transport::ws::connect_ws(socket, ws_cfg, server, port).await?,
        )),
        (None, None, Some(tls_cfg)) => {
            let tls_stream =
                zero_transport::tls::connect_tls_upstream(socket, tls_cfg, source_dir, server)
                    .await?;
            Ok(TcpRelayStream::new(tls_stream))
        }
        (None, None, None) => Ok(TcpRelayStream::new(socket)),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "vmess: ws and grpc are mutually exclusive",
        ))),
    }
}

fn transport_key(
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
) -> Result<VmessMuxTransportKey, EngineError> {
    match (grpc, ws, tls) {
        (Some(grpc), None, tls) => Ok(VmessMuxTransportKey::Grpc {
            server_name: tls.and_then(|tls| tls.server_name.clone()),
            service_names: grpc.service_names.clone(),
        }),
        (None, Some(ws), tls) => Ok(VmessMuxTransportKey::Ws {
            server_name: tls.and_then(|tls| tls.server_name.clone()),
            path: ws.path.clone(),
        }),
        (None, None, tls) => Ok(VmessMuxTransportKey::RawTls {
            server_name: tls.and_then(|tls| tls.server_name.clone()),
        }),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "vmess: ws and grpc are mutually exclusive",
        ))),
    }
}
