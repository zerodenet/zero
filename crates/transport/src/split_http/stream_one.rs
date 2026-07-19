use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::RuntimeError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_platform_tokio::ClientStream;
use zero_traits::{AsyncSocket, SplitHttpTransportProfile};

use super::chunked::{ChunkedDecoder, DecodeStep};
use super::registry::generate_session_id;
use super::wire::{find_header_end, parse_status, validate_path};

/// Parsed XHTTP framing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpMode {
    Auto,
    PacketUp,
    StreamUp,
    StreamOne,
}

impl XhttpMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "" | "auto" => XhttpMode::Auto,
            "packet-up" => XhttpMode::PacketUp,
            "stream-up" => XhttpMode::StreamUp,
            "stream-one" => XhttpMode::StreamOne,
            _ => XhttpMode::Auto,
        }
    }

    pub fn is_single_connection(self) -> bool {
        matches!(self, XhttpMode::Auto | XhttpMode::StreamOne)
    }
}

/// Single-connection bidirectional XHTTP stream (`stream-one` mode).
pub struct XhttpStreamOne<S> {
    inner: S,
    decoder: ChunkedDecoder,
    write_finished: bool,
}

pub async fn connect_xhttp_stream_one<S, TProfile>(
    stream: S,
    config: &TProfile,
) -> Result<XhttpStreamOne<S>, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: SplitHttpTransportProfile + ?Sized,
{
    let host = config.host().unwrap_or("localhost");
    let path = config.path();
    let session_id = generate_session_id();
    let mut stream = stream;
    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         X-Session-Id: {session_id}\r\n\
         Transfer-Encoding: chunked\r\n\
         Content-Type: application/octet-stream\r\n\
         \r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(RuntimeError::Io)?;
    stream.flush().await.map_err(RuntimeError::Io)?;

    let mut buf = vec![0u8; 8192];
    let mut total = 0;
    let head_end = loop {
        let n = stream
            .read(&mut buf[total..])
            .await
            .map_err(RuntimeError::Io)?;
        if n == 0 {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "xhttp stream-one: unexpected EOF reading response",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "xhttp stream-one: response headers too large",
            )));
        }
    };

    let status = parse_status(&buf[..head_end]);
    if status != Some(200) {
        return Err(RuntimeError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("xhttp stream-one: expected 200, got {status:?}"),
        )));
    }

    let prefetched = buf[head_end..total].to_vec();
    Ok(XhttpStreamOne {
        inner: stream,
        decoder: ChunkedDecoder::with_prefetched(prefetched),
        write_finished: false,
    })
}

pub async fn accept_xhttp_stream_one<S, TProfile>(
    stream: S,
    config: &TProfile,
) -> Result<XhttpStreamOne<S>, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: SplitHttpTransportProfile + ?Sized,
{
    let mut stream = stream;
    let mut buf = vec![0u8; 8192];
    let mut total = 0;
    let head_end = loop {
        let n = stream
            .read(&mut buf[total..])
            .await
            .map_err(RuntimeError::Io)?;
        if n == 0 {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "xhttp stream-one accept: unexpected EOF before request headers",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "xhttp stream-one accept: request headers too large",
            )));
        }
    };

    validate_path(&buf[..head_end], config.path())?;
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
        .await
        .map_err(RuntimeError::Io)?;
    stream.flush().await.map_err(RuntimeError::Io)?;

    let prefetched = buf[head_end..total].to_vec();
    Ok(XhttpStreamOne {
        inner: stream,
        decoder: ChunkedDecoder::with_prefetched(prefetched),
        write_finished: false,
    })
}

impl<S> AsyncRead for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            match this.decoder.try_decode(buf)? {
                DecodeStep::Done => return Poll::Ready(Ok(())),
                DecodeStep::NeedsMore => {
                    let mut tmp = [0u8; 8192];
                    let mut rb = ReadBuf::new(&mut tmp);
                    match Pin::new(&mut this.inner).poll_read(cx, &mut rb) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                        Poll::Ready(Ok(())) => {
                            if rb.filled().is_empty() {
                                return Poll::Ready(Ok(()));
                            }
                            this.decoder.feed(rb.filled());
                        }
                    }
                }
            }
        }
    }
}

impl<S> AsyncWrite for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if buf.is_empty() || self.write_finished {
            return Poll::Ready(Ok(0));
        }
        let this = self.get_mut();
        let header = format!("{:x}\r\n", buf.len());
        let frame: Vec<u8> = header
            .as_bytes()
            .iter()
            .chain(buf.iter())
            .chain(b"\r\n".iter())
            .copied()
            .collect();

        match Pin::new(&mut this.inner).poll_write(cx, &frame) {
            Poll::Ready(Ok(written)) => {
                let data_written = if written >= header.len() + 2 {
                    buf.len().min(written - header.len() - 2)
                } else {
                    0
                };
                Poll::Ready(Ok(data_written))
            }
            other => other,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.write_finished {
            return Poll::Ready(Ok(()));
        }
        this.write_finished = true;
        match Pin::new(&mut this.inner).poll_write(cx, b"0\r\n\r\n") {
            Poll::Ready(Ok(_)) => {
                let _ = Pin::new(&mut this.inner).poll_flush(cx);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S> AsyncSocket for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
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

impl<S> ClientStream for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "XHTTP stream-one stream does not expose local_addr",
        ))
    }
}
