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

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use zero_core::{Address, Error};

use crate::MuxCrypto;

// ── Pool key types ──

/// Identifies a unique upstream endpoint including transport.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub server: String,
    pub port: u16,
    pub uuid: [u8; 16],
    pub transport: TransportKey,
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
    crate::encode_new_stream(network, port, address)
}

pub fn encode_mux_data_frame(session_id: u16, payload: &[u8]) -> Vec<u8> {
    crate::encode_data_frame(session_id, payload)
}

pub fn encode_mux_end_frame(session_id: u16) -> Vec<u8> {
    crate::encode_end_frame(session_id)
}
