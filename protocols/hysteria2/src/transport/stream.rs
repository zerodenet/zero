use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_traits::AsyncSocket;

pub struct Hysteria2Stream {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl Hysteria2Stream {
    pub fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self { send, recv }
    }
}

impl AsyncRead for Hysteria2Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for Hysteria2Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.send)
            .poll_write(cx, buf)
            .map_err(io::Error::other)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_flush(cx)
            .map_err(io::Error::other)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_shutdown(cx)
            .map_err(io::Error::other)
    }
}

impl AsyncSocket for Hysteria2Stream {
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
