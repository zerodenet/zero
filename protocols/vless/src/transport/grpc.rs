// gRPC transport — grpc.rs
//
// Bidirectional streaming over HTTP/2 with gRPC wire format.
// Data framing: [1 byte: compressed_flag(0)] [4 bytes: length BE] [payload]
//
// Max frame payload: 16384 bytes (within single TLS record boundary).

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Method, Request, Response};
use rand::seq::IndexedRandom;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;

use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use zero_platform_tokio::ClientStream;

const GRPC_HEADER_LEN: usize = 5;
const GRPC_MAX_PAYLOAD: usize = 16384;

/// gRPC frame header: [compressed(1)] [length(4 BE)]
fn grpc_frame_header(len: usize) -> [u8; GRPC_HEADER_LEN] {
    let mut header = [0u8; GRPC_HEADER_LEN];
    header[0] = 0; // uncompressed
    header[1..5].copy_from_slice(&(len as u32).to_be_bytes());
    header
}

fn parse_grpc_frame_header(header: &[u8; GRPC_HEADER_LEN]) -> (bool, usize) {
    let compressed = header[0] != 0;
    let len = u32::from_be_bytes([header[1], header[2], header[3], header[4]]) as usize;
    (compressed, len)
}

/// Bidirectional gRPC stream wrapping h2 send/recv halves via internal channels.
pub struct GrpcStream {
    read_rx: mpsc::Receiver<Vec<u8>>,
    write_tx: mpsc::Sender<Vec<u8>>,
    read_buffer: Vec<u8>,
    read_offset: usize,
    write_closed: bool,
}

impl GrpcStream {
    fn new(read_rx: mpsc::Receiver<Vec<u8>>, write_tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            read_rx,
            write_tx,
            read_buffer: Vec::new(),
            read_offset: 0,
            write_closed: false,
        }
    }
}

// ── client (outbound) connect ──

pub async fn connect_grpc<S>(
    stream: S,
    service_names: &[String],
) -> Result<GrpcStream, EngineError>
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

    // Pick a random service name
    let name = service_names
        .choose(&mut rand::rng())
        .map(|s| s.as_str())
        .unwrap_or("/v2ray.core.proxy.vless.encap.GrpcService/Tun");

    let request = Request::builder()
        .method(Method::POST)
        .uri(name)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .body(())
        .map_err(|e| EngineError::Io(io::Error::other(format!("grpc request build: {e}"))))?;

    // h2 0.4: send_request is synchronous, returns (ResponseFuture, SendStream)
    let (resp_future, send_stream) = h2
        .send_request(request, false)
        .map_err(|e| EngineError::Io(io::Error::other(format!("grpc send request: {e}"))))?;

    let resp = resp_future
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("grpc response: {e}"))))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("grpc server returned {status}"),
        )));
    }

    let recv_stream = resp.into_body();

    build_grpc_stream(send_stream, recv_stream)
}

// ── server (inbound) accept ──

pub async fn accept_grpc<S>(
    stream: S,
    expected_services: &[String],
) -> Result<GrpcStream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut conn = h2::server::handshake(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 server handshake: {e}"))))?;

    // Accept the first request
    let (request, mut respond) = conn
        .accept()
        .await
        .ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "h2 connection closed before request",
            ))
        })?
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 accept: {e}"))))?;

    let path = request.uri().path();
    if !expected_services.iter().any(|s| s == path) {
        let mut resp = Response::new(());
        *resp.status_mut() = http::StatusCode::NOT_FOUND;
        respond
            .send_response(resp, true)
            .map_err(|e| EngineError::Io(io::Error::other(format!("h2 respond: {e}"))))?;
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("grpc path mismatch: got {path}"),
        )));
    }

    let mut resp = Response::new(());
    resp.headers_mut()
        .insert("content-type", "application/grpc".parse().unwrap());

    let mut send_stream = respond
        .send_response(resp, false)
        .map_err(|e| EngineError::Io(io::Error::other(format!("h2 respond: {e}"))))?;

    let mut recv_stream = request.into_body();

    build_grpc_stream(send_stream, recv_stream)
}

// ── common gRPC stream builder ──

fn build_grpc_stream(
    mut send_stream: h2::SendStream<Bytes>,
    mut recv_stream: h2::RecvStream,
) -> Result<GrpcStream, EngineError> {
    let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(32);
    let (read_tx, read_rx) = mpsc::channel::<Vec<u8>>(32);

    // Write relay: mpsc → gRPC frames → h2 send_stream
    tokio::spawn(async move {
        while let Some(data) = write_rx.recv().await {
            for chunk in data.chunks(GRPC_MAX_PAYLOAD) {
                let header = grpc_frame_header(chunk.len());
                let mut frame = Vec::with_capacity(GRPC_HEADER_LEN + chunk.len());
                frame.extend_from_slice(&header);
                frame.extend_from_slice(chunk);
                if send_stream
                    .send_data(Bytes::from(frame), false)
                    .is_err()
                {
                    return;
                }
            }
        }
        // End of stream — send empty frame with END_STREAM
        let _ = send_stream.send_data(Bytes::new(), true);
    });

    // Read relay: h2 recv_stream → gRPC frame parse → mpsc
    tokio::spawn(async move {
        use h2::RecvStream;
        let mut frame_buf = Vec::new();
        let mut header_buf = [0u8; GRPC_HEADER_LEN];
        let mut header_pos = 0;
        let mut expecting_payload: Option<usize> = None;

        loop {
            match recv_stream.data().await {
                Some(Ok(data)) => {
                    frame_buf.extend_from_slice(&data);
                    let _ = recv_stream.flow_control().release_capacity(data.len()).ok();

                    loop {
                        if let Some(payload_len) = expecting_payload {
                            if frame_buf.len() >= payload_len {
                                let payload = frame_buf[..payload_len].to_vec();
                                frame_buf.drain(..payload_len);
                                expecting_payload = None;
                                header_pos = 0;
                                if read_tx.send(payload).await.is_err() {
                                    return;
                                }
                            } else {
                                break;
                            }
                        } else {
                            let needed = GRPC_HEADER_LEN - header_pos;
                            let available = frame_buf.len().min(needed);
                            if available > 0 {
                                header_buf[header_pos..header_pos + available]
                                    .copy_from_slice(&frame_buf[..available]);
                                header_pos += available;
                                frame_buf.drain(..available);
                            }
                            if header_pos == GRPC_HEADER_LEN {
                                let (compressed, len) = parse_grpc_frame_header(&header_buf);
                                if compressed {
                                    return;
                                }
                                if len == 0 {
                                    return;
                                }
                                if len > 1024 * 1024 {
                                    return;
                                }
                                expecting_payload = Some(len);
                            } else {
                                break;
                            }
                        }
                    }
                }
                Some(Err(_)) => return,
                None => return,
            }
        }
    });

    Ok(GrpcStream::new(read_rx, write_tx))
}

// ── AsyncRead / AsyncWrite / AsyncSocket / ClientStream impls ──

impl AsyncRead for GrpcStream {
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

impl AsyncWrite for GrpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        if self.write_closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "grpc write side closed",
            )));
        }
        match self.write_tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Full(_)) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "grpc write side closed",
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

impl AsyncSocket for GrpcStream {
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

impl ClientStream for GrpcStream {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "GrpcStream does not expose local_addr",
        ))
    }
}
