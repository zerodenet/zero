// HTTP/2 transport — h2.rs
//
// Raw DATA frames over HTTP/2 (no gRPC framing).
// Simpler than gRPC transport: bytes flow directly in DATA frames.

use std::io;
#[cfg(feature = "inbound-socks5")]
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Method, Request, Response};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;

use zero_config::H2Config;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use super::ClientStream;

/// Bidirectional HTTP/2 stream.
pub(crate) struct H2Stream {
    read_rx: mpsc::Receiver<Vec<u8>>,
    write_tx: mpsc::Sender<Vec<u8>>,
    read_buffer: Vec<u8>,
    read_offset: usize,
}

impl H2Stream {
    fn new(read_rx: mpsc::Receiver<Vec<u8>>, write_tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            read_rx,
            write_tx,
            read_buffer: Vec::new(),
            read_offset: 0,
        }
    }
}

// ── client (outbound) connect ──

#[cfg(feature = "outbound-vless")]
pub(crate) async fn connect_h2<S>(
    stream: S,
    h2_config: &H2Config,
    server: &str,
    port: u16,
) -> Result<H2Stream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut h2, conn) = h2::client::handshake(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 client handshake: {e}"))))?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::warn!(error = %e, "h2 client connection error");
        }
    });

    let host = h2_config
        .host
        .clone()
        .unwrap_or_else(|| format!("{server}:{port}"));
    let path = if h2_config.path.starts_with('/') {
        h2_config.path.clone()
    } else {
        format!("/{}", h2_config.path)
    };

    let request = Request::builder()
        .method(Method::POST)
        .uri(&path)
        .header("host", &host)
        .header("content-type", "application/octet-stream")
        .body(())
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 request build: {e}"))))?;

    let (resp_future, send_stream) = h2
        .send_request(request, false)
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 send request: {e}"))))?;

    let resp = resp_future
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 response: {e}"))))?;

    if !resp.status().is_success() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("h2 server returned {}", resp.status()),
        )));
    }

    let recv_stream = resp.into_body();

    build_h2_stream(send_stream, recv_stream)
}

// ── common H2 stream builder ──

fn build_h2_stream(
    mut send_stream: h2::SendStream<Bytes>,
    mut recv_stream: h2::RecvStream,
) -> Result<H2Stream, EngineError> {
    let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(32);
    let (read_tx, read_rx) = mpsc::channel::<Vec<u8>>(32);

    // Write relay: mpsc → h2 DATA frames
    tokio::spawn(async move {
        while let Some(data) = write_rx.recv().await {
            if send_stream.send_data(Bytes::from(data), false).is_err() {
                return;
            }
        }
        let _ = send_stream.send_data(Bytes::new(), true);
    });

    // Read relay: h2 DATA frames → mpsc
    tokio::spawn(async move {
        loop {
            match recv_stream.data().await {
                Some(Ok(data)) => {
                    let _ = recv_stream.flow_control().release_capacity(data.len()).ok();
                    if read_tx.send(data.to_vec()).await.is_err() {
                        return;
                    }
                }
                Some(Err(_)) => return,
                None => return,
            }
        }
    });

    Ok(H2Stream::new(read_rx, write_tx))
}

// ── AsyncRead / AsyncWrite / AsyncSocket / ClientStream ──

impl AsyncRead for H2Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.read_offset < self.read_buffer.len() {
            let available = self.read_buffer.len() - self.read_offset;
            let to_copy = available.min(buf.remaining());
            buf.put_slice(&self.read_buffer[self.read_offset..self.read_offset + to_copy]);
            self.read_offset += to_copy;
            if self.read_offset >= self.read_buffer.len() {
                self.read_buffer.clear();
                self.read_offset = 0;
            }
            return Poll::Ready(Ok(()));
        }

        match self.read_rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let to_copy = data.len().min(buf.remaining());
                buf.put_slice(&data[..to_copy]);
                if to_copy < data.len() {
                    self.read_buffer = data;
                    self.read_offset = to_copy;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for H2Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match self.write_tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Full(_)) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "h2 write side closed",
                )))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncSocket for H2Stream {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(self, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(self, buf).await?;
        AsyncWriteExt::flush(self).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(self).await
    }
}

impl ClientStream for H2Stream {
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "H2Stream does not expose local_addr",
        ))
    }
}
