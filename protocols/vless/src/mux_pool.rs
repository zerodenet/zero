//! VLESS MUX connection pool shared types.
//!
//! Types that are pure VLESS MUX protocol logic live here.
//! Connection establishment (raw TCP, transport wrapping) stays in the
//! proxy crate which owns the I/O infrastructure.

use core::future::Future;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use zero_core::{Address, Error};

use crate::mux_crypto::MuxCrypto;

// ── Pool key types ──

/// Identifies a unique upstream endpoint including transport.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PoolKey {
    server: String,
    port: u16,
    identity: MuxIdentity,
    transport: TransportKey,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct MuxIdentity {
    uuid: [u8; 16],
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TransportKey {
    Raw,
    Tls {
        server_name: Option<String>,
    },
    Reality {
        public_key: String,
        server_name: String,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct MuxTransportProfile<'a> {
    tls_server_name: Option<&'a str>,
    reality_public_key: Option<&'a str>,
    reality_server_name: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedMuxTransportProfile {
    tls_server_name: Option<String>,
    reality_public_key: Option<String>,
    reality_server_name: Option<String>,
}

#[derive(Clone)]
pub struct MuxConnectionPool {
    pool: Arc<Mutex<HashMap<PoolKey, Arc<MuxPoolConn>>>>,
}

struct PoolKeyConfig {
    server: String,
    port: u16,
    identity: MuxIdentity,
    tls_server_name: Option<String>,
    reality_public_key: Option<String>,
    reality_server_name: Option<String>,
}

impl PoolKeyConfig {
    fn new(server: impl Into<String>, port: u16, identity: MuxIdentity) -> Self {
        Self {
            server: server.into(),
            port,
            identity,
            tls_server_name: None,
            reality_public_key: None,
            reality_server_name: None,
        }
    }

    fn with_tls_server_name(mut self, server_name: Option<&str>) -> Self {
        self.tls_server_name = server_name.map(ToOwned::to_owned);
        self
    }

    fn with_reality(mut self, public_key: Option<&str>, server_name: Option<&str>) -> Self {
        self.reality_public_key = public_key.map(ToOwned::to_owned);
        self.reality_server_name = server_name.map(ToOwned::to_owned);
        self
    }

    fn into_pool_key(self) -> PoolKey {
        PoolKey::from_config_parts(
            self.server,
            self.port,
            self.identity,
            self.tls_server_name.as_deref(),
            self.reality_public_key.as_deref(),
            self.reality_server_name.as_deref(),
        )
    }
}

fn transport_key_from_config(
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

pub(crate) fn pool_key_from_transport_config(
    server: &str,
    port: u16,
    identity: MuxIdentity,
    profile: MuxTransportProfile<'_>,
) -> PoolKey {
    PoolKeyConfig::new(server, port, identity)
        .with_tls_server_name(profile.tls_server_name)
        .with_reality(profile.reality_public_key, profile.reality_server_name)
        .into_pool_key()
}

impl<'a> MuxTransportProfile<'a> {
    pub const fn new(
        tls_server_name: Option<&'a str>,
        reality_public_key: Option<&'a str>,
        reality_server_name: Option<&'a str>,
    ) -> Self {
        Self {
            tls_server_name,
            reality_public_key,
            reality_server_name,
        }
    }
}

impl OwnedMuxTransportProfile {
    pub(crate) fn new(
        tls_server_name: Option<String>,
        reality_public_key: Option<String>,
        reality_server_name: Option<String>,
    ) -> Self {
        Self {
            tls_server_name,
            reality_public_key,
            reality_server_name,
        }
    }

    pub(crate) fn as_borrowed(&self) -> MuxTransportProfile<'_> {
        MuxTransportProfile::new(
            self.tls_server_name.as_deref(),
            self.reality_public_key.as_deref(),
            self.reality_server_name.as_deref(),
        )
    }
}

impl MuxIdentity {
    pub(crate) fn from_uuid(uuid: [u8; 16]) -> Self {
        Self { uuid }
    }

    fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }
}

impl PoolKey {
    fn from_identity(
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

    fn from_config_parts(
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

    fn uuid(&self) -> &[u8; 16] {
        self.identity.uuid()
    }

    async fn establish_mux_connection<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: zero_traits::AsyncSocket,
    {
        establish_outbound_mux_connection(stream, self.uuid()).await
    }

    fn into_pool_conn<S>(self, stream: S, max_concurrency: u32) -> MuxPoolConn
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        MuxPoolConn::new(stream, self.uuid(), max_concurrency)
    }
}

pub async fn establish_outbound_mux_connection<S>(
    stream: &mut S,
    id: &[u8; 16],
) -> Result<(), Error>
where
    S: zero_traits::AsyncSocket,
{
    crate::outbound::establish_outbound_mux_connection(stream, id).await
}

impl Default for MuxConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MuxConnectionPool {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MuxConnectionPool")
            .field(
                "entries",
                &self.pool.lock().expect("mux pool lock poisoned").len(),
            )
            .finish()
    }
}

impl MuxConnectionPool {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn evict_all(&self) {
        self.pool.lock().expect("mux pool lock poisoned").clear();
    }

    pub(crate) async fn open_tcp_stream<S, OpenStream, OpenStreamFut, E>(
        &self,
        key: PoolKey,
        max_concurrency: u32,
        port: u16,
        address: &Address,
        open_stream: OpenStream,
    ) -> Result<impl AsyncRead + AsyncWrite + Send + Unpin + 'static, E>
    where
        S: zero_traits::AsyncSocket + AsyncRead + AsyncWrite + Unpin + Send + 'static,
        OpenStream: FnOnce() -> OpenStreamFut,
        OpenStreamFut: Future<Output = Result<S, E>>,
        E: From<Error>,
    {
        let conn = self
            .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
                let mut stream = match open_stream().await {
                    Ok(stream) => stream,
                    Err(error) => return Err(error),
                };
                if let Err(error) = key.establish_mux_connection(&mut stream).await {
                    return Err(E::from(error));
                }
                Ok(key.into_pool_conn(stream, max_concurrency))
            })
            .await?;
        conn.open_tcp_stream(port, address).map_err(E::from)
    }

    pub(crate) async fn open_udp_stream<S, OpenStream, OpenStreamFut, E>(
        &self,
        key: PoolKey,
        max_concurrency: u32,
        open_stream: OpenStream,
    ) -> Result<
        (
            u16,
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ),
        E,
    >
    where
        S: zero_traits::AsyncSocket + AsyncRead + AsyncWrite + Unpin + Send + 'static,
        OpenStream: FnOnce() -> OpenStreamFut,
        OpenStreamFut: Future<Output = Result<S, E>>,
        E: From<Error>,
    {
        let conn = self
            .get_or_create_conn(key, max_concurrency, |key, max_concurrency| async move {
                let mut stream = match open_stream().await {
                    Ok(stream) => stream,
                    Err(error) => return Err(error),
                };
                if let Err(error) = key.establish_mux_connection(&mut stream).await {
                    return Err(E::from(error));
                }
                Ok(key.into_pool_conn(stream, max_concurrency))
            })
            .await?;
        conn.open_udp_stream().map_err(E::from)
    }

    async fn get_or_create_conn<F, Fut, E>(
        &self,
        key: PoolKey,
        max_concurrency: u32,
        create_conn: F,
    ) -> Result<Arc<MuxPoolConn>, E>
    where
        F: FnOnce(PoolKey, u32) -> Fut,
        Fut: Future<Output = Result<MuxPoolConn, E>>,
    {
        let cached = {
            let pool = self.pool.lock().expect("mux pool lock poisoned");
            pool.get(&key).cloned()
        };

        match cached {
            Some(conn)
                if *conn.active.lock().expect("mux conn active lock poisoned")
                    < conn.max_concurrency as usize =>
            {
                Ok(conn)
            }
            _ => {
                let conn = Arc::new(create_conn(key.clone(), max_concurrency).await?);
                self.pool
                    .lock()
                    .expect("mux pool lock poisoned")
                    .insert(key, conn.clone());
                Ok(conn)
            }
        }
    }
}

// ── Pool connection ──

/// A single MUX connection to an upstream, shared by multiple streams.
struct MuxPoolConn {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    next_id: Mutex<u16>,
    active: Mutex<usize>,
    max_concurrency: u32,
    crypto: Option<Arc<Mutex<MuxCrypto>>>,
}

impl MuxPoolConn {
    fn new<S>(stream: S, uuid: &[u8; 16], max_concurrency: u32) -> Self
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

    fn open_tcp_stream(
        self: &Arc<Self>,
        port: u16,
        address: &Address,
    ) -> Result<impl AsyncRead + AsyncWrite + Send + Unpin + 'static, Error> {
        let sid = self.allocate_stream_id();
        let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        self.streams.lock().unwrap().insert(sid, down_tx);

        let req = encode_mux_new_stream(crate::mux::NETWORK_TCP, port, address)?;
        self.write_tx
            .send(req)
            .map_err(|_| Error::Io("failed to write VLESS MUX new stream request"))?;

        spawn_mux_upload_relay(self.clone(), sid, up_rx, false);

        Ok(MuxStreamRelay {
            up_tx,
            sid,
            down_rx: Some(down_rx),
            conn: self.clone(),
        })
    }

    fn open_udp_stream(
        self: &Arc<Self>,
    ) -> Result<
        (
            u16,
            mpsc::UnboundedSender<Vec<u8>>,
            mpsc::UnboundedReceiver<Vec<u8>>,
        ),
        Error,
    > {
        let sid = self.allocate_stream_id();
        let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (down_tx, down_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        self.streams.lock().unwrap().insert(sid, down_tx);

        let req = encode_mux_new_stream(crate::mux::NETWORK_UDP, 0, &Address::Ipv4([0, 0, 0, 0]))?;
        self.write_tx
            .send(req)
            .map_err(|_| Error::Io("failed to write VLESS MUX UDP stream request"))?;

        spawn_mux_upload_relay(self.clone(), sid, up_rx, true);

        Ok((sid, up_tx, down_rx))
    }

    fn allocate_stream_id(&self) -> u16 {
        let sid = {
            let mut next = self.next_id.lock().unwrap();
            let s = *next;
            *next = next.wrapping_add(1);
            if *next == 0 {
                *next = 1;
            }
            s
        };
        *self.active.lock().unwrap() += 1;
        sid
    }
}

// ── MUX stream relay ──

/// A single MUX stream — implements `AsyncRead` + `AsyncWrite` over the
/// shared MUX connection.
struct MuxStreamRelay {
    up_tx: mpsc::UnboundedSender<Vec<u8>>,
    sid: u16,
    down_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    conn: Arc<MuxPoolConn>,
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

fn new_mux_crypto(uuid: &[u8; 16]) -> Option<Arc<Mutex<MuxCrypto>>> {
    Some(Arc::new(Mutex::new(MuxCrypto::new(uuid))))
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
fn encrypt_mux_payload(
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
fn decrypt_mux_payload(
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

fn encode_mux_new_stream(network: u8, port: u16, address: &Address) -> Result<Vec<u8>, Error> {
    crate::mux::encode_new_stream(network, port, address)
}

fn encode_mux_data_frame(session_id: u16, payload: &[u8]) -> Vec<u8> {
    crate::mux::encode_data_frame(session_id, payload)
}

fn encode_mux_end_frame(session_id: u16) -> Vec<u8> {
    crate::mux::encode_end_frame(session_id)
}
