// HTTP/2 transport 鈥?h2.rs
//
// Raw DATA frames over HTTP/2 (no gRPC framing).
// Simpler than gRPC transport: bytes flow directly in DATA frames.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Method, Request, Response};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;

use crate::RuntimeError;
use zero_traits::{AsyncSocket, H2TransportProfile};

use zero_platform_tokio::ClientStream;

/// Bidirectional HTTP/2 stream.
pub struct H2Stream {
    read_rx: mpsc::Receiver<Vec<u8>>,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    read_buffer: Vec<u8>,
    read_offset: usize,
}

impl H2Stream {
    fn new(read_rx: mpsc::Receiver<Vec<u8>>, write_tx: mpsc::UnboundedSender<Vec<u8>>) -> Self {
        Self {
            read_rx,
            write_tx,
            read_buffer: Vec::new(),
            read_offset: 0,
        }
    }
}

// 鈹€鈹€ client (outbound) connect 鈹€鈹€

pub async fn connect_h2<S, TProfile>(
    stream: S,
    h2_config: &TProfile,
    server: &str,
    port: u16,
) -> Result<H2Stream, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: H2TransportProfile + ?Sized,
{
    let (mut h2, conn) = h2::client::handshake(stream)
        .await
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 client handshake: {e}"))))?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::warn!(error = %e, "h2 client connection error");
        }
    });

    let host = h2_config
        .host()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{server}:{port}"));
    let path = if h2_config.path().starts_with('/') {
        h2_config.path().to_owned()
    } else {
        format!("/{}", h2_config.path())
    };

    let request = Request::builder()
        .method(Method::POST)
        .uri(&path)
        .header("host", &host)
        .header("content-type", "application/octet-stream")
        .body(())
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 request build: {e}"))))?;

    let (resp_future, send_stream) = h2
        .send_request(request, false)
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 send request: {e}"))))?;

    let resp = resp_future
        .await
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 response: {e}"))))?;

    if !resp.status().is_success() {
        return Err(RuntimeError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("h2 server returned {}", resp.status()),
        )));
    }

    let recv_stream = resp.into_body();

    build_h2_stream(send_stream, recv_stream)
}

// 鈹€鈹€ server (inbound) accept 鈹€鈹€

pub async fn accept_h2<S, TProfile>(
    stream: S,
    h2_config: &TProfile,
) -> Result<H2Stream, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: H2TransportProfile + ?Sized,
{
    let mut conn = h2::server::handshake(stream)
        .await
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 server handshake: {e}"))))?;

    let (request, mut respond) = conn
        .accept()
        .await
        .ok_or_else(|| {
            RuntimeError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "h2 connection closed before request",
            ))
        })?
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 accept: {e}"))))?;

    let expected_path = if h2_config.path().starts_with('/') {
        h2_config.path()
    } else {
        "/"
    };
    let got_path = request.uri().path();
    if got_path != expected_path {
        let mut resp = Response::new(());
        *resp.status_mut() = http::StatusCode::NOT_FOUND;
        respond
            .send_response(resp, true)
            .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 respond: {e}"))))?;
        return Err(RuntimeError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("h2 path mismatch: expected {expected_path}, got {got_path}"),
        )));
    }

    let mut resp = Response::new(());
    resp.headers_mut()
        .insert("content-type", "application/octet-stream".parse().unwrap());

    let send_stream = respond
        .send_response(resp, false)
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("h2 respond: {e}"))))?;

    let recv_stream = request.into_body();

    build_h2_stream(send_stream, recv_stream)
}

// 鈹€鈹€ common H2 stream builder 鈹€鈹€

fn build_h2_stream(
    mut send_stream: h2::SendStream<Bytes>,
    mut recv_stream: h2::RecvStream,
) -> Result<H2Stream, RuntimeError> {
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (read_tx, read_rx) = mpsc::channel::<Vec<u8>>(32);

    // Write relay: mpsc 鈫?h2 DATA frames
    tokio::spawn(async move {
        while let Some(data) = write_rx.recv().await {
            if send_stream.send_data(Bytes::from(data), false).is_err() {
                return;
            }
        }
        let _ = send_stream.send_data(Bytes::new(), true);
    });

    // Read relay: h2 DATA frames 鈫?mpsc
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

// 鈹€鈹€ AsyncRead / AsyncWrite / AsyncSocket / ClientStream 鈹€鈹€

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
        match self.write_tx.send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(_) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "h2 write side closed",
            ))),
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
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "H2Stream does not expose local_addr",
        ))
    }
}
