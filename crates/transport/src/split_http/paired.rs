use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_platform_tokio::ClientStream;
use zero_traits::AsyncSocket;

use super::chunked::{ChunkedDecoder, DecodeStep};

/// Bidirectional stream combining POST body (read) and GET response (write).
///
/// - `AsyncRead`: reads from `reader`, decoding chunked transfer encoding
/// - `AsyncWrite`: writes to `writer`, encoding as chunked transfer encoding
///
/// In stream-one mode `R = W = S` (same TCP connection).
/// In multi-connection mode they differ (POST TCP and GET TCP).
pub struct SplitHttpPairedStream<R, W> {
    reader: R,
    writer: W,
    decoder: ChunkedDecoder,
    write_finished: bool,
}

/// Convenience alias: same connection for both directions (stream-one mode).
pub type SplitHttpStream<S> = SplitHttpPairedStream<S, S>;

impl<R, W> SplitHttpPairedStream<R, W> {
    pub(super) fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            decoder: ChunkedDecoder::new(),
            write_finished: false,
        }
    }

    pub(super) fn new_with_prefetched(reader: R, writer: W, prefetched: Vec<u8>) -> Self {
        Self {
            reader,
            writer,
            decoder: ChunkedDecoder::with_prefetched(prefetched),
            write_finished: false,
        }
    }
}

impl<R, W> AsyncRead for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: Unpin,
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
                    match Pin::new(&mut this.reader).poll_read(cx, &mut rb) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Ready(Ok(())) => {
                            let filled = rb.filled();
                            if filled.is_empty() {
                                return Poll::Ready(Ok(()));
                            }
                            this.decoder.feed(filled);
                        }
                    }
                }
            }
        }
    }
}

impl<R, W> AsyncWrite for SplitHttpPairedStream<R, W>
where
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    R: Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        if buf.is_empty() || self.write_finished {
            return Poll::Ready(Ok(0));
        }

        let this = self.get_mut();
        let inner = unsafe { Pin::new_unchecked(&mut this.writer) };
        let header = format!("{:x}\r\n", buf.len());
        let frame: Vec<u8> = header
            .as_bytes()
            .iter()
            .chain(buf.iter())
            .chain(b"\r\n".iter())
            .copied()
            .collect();

        match inner.poll_write(cx, &frame) {
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

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let inner = unsafe { Pin::new_unchecked(&mut self.writer) };
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        if this.write_finished {
            return Poll::Ready(Ok(()));
        }
        this.write_finished = true;
        let write_res = {
            let inner = unsafe { Pin::new_unchecked(&mut this.writer) };
            inner.poll_write(cx, b"0\r\n\r\n")
        };
        match write_res {
            Poll::Ready(Ok(_)) => {
                let inner = unsafe { Pin::new_unchecked(&mut this.writer) };
                let _ = inner.poll_flush(cx);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<R, W> AsyncSocket for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move { AsyncReadExt::read(self, buf).await }
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            AsyncWriteExt::write_all(self, buf).await?;
            AsyncWriteExt::flush(self).await
        }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::shutdown(self).await }
    }
}

impl<R, W> ClientStream for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SplitHttp stream does not expose local_addr",
        ))
    }
}
