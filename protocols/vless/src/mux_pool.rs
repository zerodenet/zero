//! VLESS MUX connection pool shared types.
//!
//! Types that are pure VLESS MUX protocol logic live here.
//! Connection establishment (raw TCP, transport wrapping) stays in the
//! proxy crate which owns the I/O infrastructure.

use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use zero_core::{Address, Error};

use crate::MuxCrypto;

// ── Pool key types ──

/// Identifies a unique upstream endpoint including transport.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub server: String,
    pub port: u16,
    identity: MuxIdentity,
    pub transport: TransportKey,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MuxIdentity {
    uuid: [u8; 16],
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TransportKey {
    Raw,
    Tls {
        server_name: Option<String>,
    },
    Reality {
        public_key: String,
        server_name: String,
    },
}

pub fn transport_key_from_config(
    tls_server_name: Option<&str>,
    reality_public_key: Option<&str>,
    reality_server_name: Option<&str>,
    fallback_server: &str,
) -> TransportKey {
    match (tls_server_name, reality_public_key, reality_server_name) {
        (Some(server_name), None, _) => TransportKey::Tls {
            server_name: Some(server_name.to_owned()),
        },
        (None, Some(public_key), server_name) => TransportKey::Reality {
            public_key: public_key.to_owned(),
            server_name: server_name.unwrap_or(fallback_server).to_owned(),
        },
        _ => TransportKey::Raw,
    }
}

impl MuxIdentity {
    pub fn from_uuid(uuid: [u8; 16]) -> Self {
        Self { uuid }
    }

    pub fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }
}

impl PoolKey {
    pub fn from_identity(
        server: String,
        port: u16,
        identity: MuxIdentity,
        transport: TransportKey,
    ) -> Self {
        Self {
            server,
            port,
            identity,
            transport,
        }
    }

    pub fn from_config_parts(
        server: String,
        port: u16,
        identity: MuxIdentity,
        tls_server_name: Option<&str>,
        reality_public_key: Option<&str>,
        reality_server_name: Option<&str>,
    ) -> Self {
        let transport = transport_key_from_config(
            tls_server_name,
            reality_public_key,
            reality_server_name,
            &server,
        );
        Self::from_identity(server, port, identity, transport)
    }

    pub fn uuid(&self) -> &[u8; 16] {
        self.identity.uuid()
    }

    pub async fn establish_mux_connection<S>(
        &self,
        stream: &mut S,
    ) -> Result<crate::mux::MuxClient, Error>
    where
        S: zero_traits::AsyncSocket,
    {
        crate::VlessOutbound
            .establish_mux(stream, self.uuid())
            .await
    }

    pub fn into_pool_conn<S>(self, stream: S, max_concurrency: u32) -> MuxPoolConn
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        MuxPoolConn::new(stream, self.uuid(), max_concurrency)
    }
}

// ── Pool connection ──

/// A single MUX connection to an upstream, shared by multiple streams.
pub struct MuxPoolConn {
    pub write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    pub next_id: Mutex<u16>,
    pub active: Mutex<usize>,
    pub max_concurrency: u32,
    pub crypto: Option<Arc<Mutex<MuxCrypto>>>,
}

impl MuxPoolConn {
    pub fn new<S>(stream: S, uuid: &[u8; 16], max_concurrency: u32) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (read_half, write_half) = tokio::io::split(stream);
        let (write_tx, write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let crypto = new_mux_crypto(uuid);

        spawn_mux_write_relay(write_half, write_rx);
        spawn_mux_read_relay(read_half, streams.clone(), crypto.clone());

        Self {
            write_tx,
            streams,
            next_id: Mutex::new(1),
            active: Mutex::new(0),
            max_concurrency,
            crypto,
        }
    }
}

pub struct MuxUdpStream {
    pub session_id: u16,
    pub up_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub down_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

// ── MUX stream relay ──

/// A single MUX stream — implements `AsyncRead` + `AsyncWrite` over the
/// shared MUX connection.
pub struct MuxStreamRelay {
    pub up_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub sid: u16,
    pub down_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    pub conn: Arc<MuxPoolConn>,
}

impl Drop for MuxStreamRelay {
    fn drop(&mut self) {
        self.conn.streams.lock().unwrap().remove(&self.sid);
        *self.conn.active.lock().unwrap() -= 1;
    }
}

impl AsyncRead for MuxStreamRelay {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let rx = match &mut self.down_rx {
            Some(rx) => rx,
            None => return Poll::Ready(Ok(())),
        };
        match rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let n = data.len().min(buf.remaining());
                buf.put_slice(&data[..n]);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => {
                self.down_rx = None;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for MuxStreamRelay {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.up_tx
            .send(buf.to_vec())
            .map(|_| Poll::Ready(Ok(buf.len())))
            .unwrap_or_else(|_| {
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "MUX upstream closed",
                )))
            })
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

// ── Crypto helpers ──

pub fn new_mux_crypto(uuid: &[u8; 16]) -> Option<Arc<Mutex<MuxCrypto>>> {
    Some(Arc::new(Mutex::new(MuxCrypto::new(uuid))))
}

pub fn open_mux_tcp_stream(
    conn: Arc<MuxPoolConn>,
    port: u16,
    address: &Address,
) -> Result<MuxStreamRelay, Error> {
    let sid = allocate_stream_id(&conn);
    let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    conn.streams.lock().unwrap().insert(sid, down_tx);

    let req = encode_mux_new_stream(crate::mux::NETWORK_TCP, port, address)?;
    conn.write_tx
        .send(req)
        .map_err(|_| Error::Io("failed to write VLESS MUX new stream request"))?;

    spawn_mux_upload_relay(conn.clone(), sid, up_rx, false);

    Ok(MuxStreamRelay {
        up_tx,
        sid,
        down_rx: Some(down_rx),
        conn,
    })
}

pub fn open_mux_udp_stream(conn: Arc<MuxPoolConn>) -> Result<MuxUdpStream, Error> {
    let sid = allocate_stream_id(&conn);
    let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    conn.streams.lock().unwrap().insert(sid, down_tx);

    let req = encode_mux_new_stream(crate::mux::NETWORK_UDP, 0, &Address::Ipv4([0, 0, 0, 0]))?;
    conn.write_tx
        .send(req)
        .map_err(|_| Error::Io("failed to write VLESS MUX UDP stream request"))?;

    spawn_mux_upload_relay(conn, sid, up_rx, true);

    Ok(MuxUdpStream {
        session_id: sid,
        up_tx,
        down_rx,
    })
}

fn allocate_stream_id(conn: &MuxPoolConn) -> u16 {
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
    sid
}

fn spawn_mux_upload_relay(
    conn: Arc<MuxPoolConn>,
    sid: u16,
    mut up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    decrement_active_on_close: bool,
) {
    let write = conn.write_tx.clone();
    let crypto = conn.crypto.clone();
    tokio::spawn(async move {
        while let Some(payload) = up_rx.recv().await {
            let payload = encrypt_mux_payload(&crypto, sid, &payload, true);
            let frame = encode_mux_data_frame(sid, &payload);
            if write.send(frame).is_err() {
                break;
            }
        }
        let close_frame = encode_mux_end_frame(sid);
        let _ = write.send(close_frame);
        if decrement_active_on_close {
            *conn.active.lock().unwrap() -= 1;
        }
    });
}

fn spawn_mux_write_relay<W>(mut writer: W, mut write_rx: mpsc::UnboundedReceiver<Vec<u8>>)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        while let Some(frame) = write_rx.recv().await {
            if writer.write_all(&frame).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
}

fn spawn_mux_read_relay<R>(
    mut reader: R,
    streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    crypto: Option<Arc<Mutex<MuxCrypto>>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; 4];
        loop {
            if reader.read_exact(&mut buf).await.is_err() {
                break;
            }
            let stream_id = u16::from_be_bytes([buf[0], buf[1]]);
            let length = u16::from_be_bytes([buf[2], buf[3]]) as usize;
            if length > 16384 {
                break;
            }
            let mut payload = vec![0u8; length];
            if length > 0 && reader.read_exact(&mut payload).await.is_err() {
                break;
            }

            if stream_id != 0 {
                let decrypted = decrypt_mux_payload(&crypto, stream_id, &payload, false);
                if let Some(decrypted_payload) = decrypted {
                    let streams = streams.lock().unwrap();
                    if let Some(tx) = streams.get(&stream_id) {
                        let _ = tx.send(decrypted_payload);
                    }
                }
            }
        }
    });
}

/// Encrypt a MUX frame payload.
/// `is_c2s`: true for client→server (upload), false for server→client.
pub fn encrypt_mux_payload(
    crypto: &Option<Arc<Mutex<MuxCrypto>>>,
    sid: u16,
    payload: &[u8],
    is_c2s: bool,
) -> Vec<u8> {
    if let Some(ref crypto) = crypto {
        if payload.is_empty() {
            return vec![];
        }
        let mut c = crypto.lock().unwrap();
        let result = if is_c2s {
            c.encrypt_c2s(sid, payload)
        } else {
            c.encrypt_s2c(sid, payload)
        };
        result.unwrap_or_else(|_| payload.to_vec())
    } else {
        payload.to_vec()
    }
}

/// Decrypt a MUX frame payload.
/// Returns `None` if decryption fails (frame should be dropped).
pub fn decrypt_mux_payload(
    crypto: &Option<Arc<Mutex<MuxCrypto>>>,
    sid: u16,
    payload: &[u8],
    is_c2s: bool,
) -> Option<Vec<u8>> {
    if let Some(ref crypto) = crypto {
        if payload.is_empty() {
            return Some(vec![]);
        }
        let mut c = crypto.lock().unwrap();
        let result = if is_c2s {
            c.decrypt_c2s(sid, payload)
        } else {
            c.decrypt_s2c(sid, payload)
        };
        result.ok()
    } else {
        Some(payload.to_vec())
    }
}

pub fn encode_mux_new_stream(network: u8, port: u16, address: &Address) -> Result<Vec<u8>, Error> {
    crate::mux::encode_new_stream(network, port, address)
}

pub fn encode_mux_data_frame(session_id: u16, payload: &[u8]) -> Vec<u8> {
    crate::mux::encode_data_frame(session_id, payload)
}

pub fn encode_mux_end_frame(session_id: u16) -> Vec<u8> {
    crate::mux::encode_end_frame(session_id)
}
