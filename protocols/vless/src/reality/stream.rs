use std::io::{self, BufRead, Write};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use super::reality_cipher_suite::{CipherSuite, DEFAULT_CIPHER_SUITES};
use super::reality_client_connection::{RealityClientConfig, RealityClientConnection};
use super::reality_util::{decode_public_key, decode_short_id};

pub struct RealityClientOptions<'a> {
    pub public_key: &'a str,
    pub short_id: &'a str,
    pub server_name: &'a str,
    pub cipher_suites: &'a [String],
}

pub async fn upgrade_reality_client<IO>(
    mut io: IO,
    options: RealityClientOptions<'_>,
) -> io::Result<RealityTlsStream<IO>>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    let config = RealityClientConfig {
        public_key: decode_public_key(options.public_key)?,
        short_id: decode_short_id(options.short_id)?,
        server_name: options.server_name.to_owned(),
        cipher_suites: parse_cipher_suites(options.cipher_suites)?,
    };

    let mut session = RealityClientConnection::new(config)?;
    perform_reality_handshake(&mut session, &mut io).await?;

    Ok(RealityTlsStream::new(io, session))
}

fn parse_cipher_suites(names: &[String]) -> io::Result<Vec<CipherSuite>> {
    if names.is_empty() {
        return Ok(DEFAULT_CIPHER_SUITES.to_vec());
    }

    names
        .iter()
        .map(|name| {
            CipherSuite::from_name(name).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported reality cipher suite `{name}`"),
                )
            })
        })
        .collect()
}

async fn perform_reality_handshake<IO>(
    session: &mut RealityClientConnection,
    io: &mut IO,
) -> io::Result<()>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    let mut read_buf = vec![0_u8; super::common::TLS_MAX_RECORD_SIZE].into_boxed_slice();
    let mut iteration = 0;

    while session.is_handshaking() || session.wants_write() {
        iteration += 1;
        if iteration > 100 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Reality handshake exceeded maximum iterations",
            ));
        }

        if session.wants_write() {
            let mut write_buf = Vec::new();
            while session.wants_write() {
                session.write_tls(&mut write_buf)?;
            }
            if !write_buf.is_empty() {
                io.write_all(&write_buf).await?;
                io.flush().await?;
            }
        }

        if session.is_handshaking() && session.wants_read() {
            let read = io.read(&mut read_buf).await?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "EOF during Reality handshake",
                ));
            }

            feed_reality_connection(session, &read_buf[..read])?;
            session.process_new_packets()?;
        }
    }

    io.flush().await
}

fn feed_reality_connection(session: &mut RealityClientConnection, data: &[u8]) -> io::Result<()> {
    let mut cursor = io::Cursor::new(data);
    let mut consumed = 0;
    while consumed < data.len() {
        let read = session.read_tls(&mut cursor)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Reality handshake input was not fully consumed",
            ));
        }
        consumed += read;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TlsState {
    Stream,
    ReadShutdown,
    WriteShutdown,
    FullyShutdown,
}

impl TlsState {
    fn shutdown_read(&mut self) {
        *self = match *self {
            Self::WriteShutdown | Self::FullyShutdown => Self::FullyShutdown,
            _ => Self::ReadShutdown,
        };
    }

    fn shutdown_write(&mut self) {
        *self = match *self {
            Self::ReadShutdown | Self::FullyShutdown => Self::FullyShutdown,
            _ => Self::WriteShutdown,
        };
    }

    fn readable(self) -> bool {
        !matches!(self, Self::ReadShutdown | Self::FullyShutdown)
    }

    fn writeable(self) -> bool {
        !matches!(self, Self::WriteShutdown | Self::FullyShutdown)
    }
}

pub struct RealityTlsStream<IO> {
    io: IO,
    session: RealityClientConnection,
    state: TlsState,
    need_flush: bool,
}

impl<IO> RealityTlsStream<IO> {
    fn new(io: IO, session: RealityClientConnection) -> Self {
        debug_assert!(!session.is_handshaking());
        Self {
            io,
            session,
            state: TlsState::Stream,
            need_flush: false,
        }
    }
}

impl<IO> RealityTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn write_tls_direct(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let mut adapter = SyncWriteAdapter {
            io: &mut self.io,
            cx,
        };
        match self.session.write_tls(&mut adapter) {
            Ok(read) => Poll::Ready(Ok(read)),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        }
    }

    fn drain_all_writes(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.session.wants_write() {
            match self.write_tls_direct(cx) {
                Poll::Ready(Ok(0)) => break,
                Poll::Ready(Ok(_)) => self.need_flush = true,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl<IO> AsyncRead for RealityTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.state.readable() {
            return Poll::Ready(Ok(()));
        }

        let mut io_pending = false;
        let mut eof = false;

        while this.state.readable() && this.session.wants_read() {
            let mut adapter = SyncReadAdapter {
                io: &mut this.io,
                cx,
            };
            match this.session.read_tls(&mut adapter) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(_) => {
                    if let Err(error) = this.session.process_new_packets() {
                        let _ = this.drain_all_writes(cx);
                        return Poll::Ready(Err(error));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    io_pending = true;
                    break;
                }
                Err(error) => return Poll::Ready(Err(error)),
            }
        }

        let mut reader = this.session.reader();
        match reader.fill_buf() {
            Ok(available) if !available.is_empty() => {
                let len = buf.remaining().min(available.len());
                buf.put_slice(&available[..len]);
                reader.consume(len);
                Poll::Ready(Ok(()))
            }
            Ok(_) => {
                this.state.shutdown_read();
                Poll::Ready(Ok(()))
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if eof {
                    this.state.shutdown_read();
                    Poll::Ready(Ok(()))
                } else if io_pending {
                    Poll::Pending
                } else {
                    let mut adapter = SyncReadAdapter {
                        io: &mut this.io,
                        cx,
                    };
                    match this.session.read_tls(&mut adapter) {
                        Ok(0) => {
                            this.state.shutdown_read();
                            Poll::Ready(Ok(()))
                        }
                        Ok(_) => {
                            let _ = this.session.process_new_packets();
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
                        Err(error) => Poll::Ready(Err(error)),
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::ConnectionAborted => {
                this.state.shutdown_read();
                Poll::Ready(Err(error))
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

impl<IO> AsyncWrite for RealityTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !self.state.writeable() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "write side is shut down",
            )));
        }

        let mut pos = 0;
        while pos < buf.len() {
            let mut would_block = false;
            match self.session.writer().write(&buf[pos..]) {
                Ok(read) => pos += read,
                Err(error) => return Poll::Ready(Err(error)),
            }

            while self.session.wants_write() {
                match self.write_tls_direct(cx) {
                    Poll::Ready(Ok(0)) | Poll::Pending => {
                        would_block = true;
                        self.need_flush = true;
                        break;
                    }
                    Poll::Ready(Ok(_)) => self.need_flush = true,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                }
            }

            return match (pos, would_block) {
                (0, true) => Poll::Pending,
                (written, true) => Poll::Ready(Ok(written)),
                (_, false) => continue,
            };
        }

        Poll::Ready(Ok(pos))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.session.writer().flush()?;

        while self.session.wants_write() {
            match self.write_tls_direct(cx) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(_)) => self.need_flush = true,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        if self.need_flush {
            match Pin::new(&mut self.io).poll_flush(cx) {
                Poll::Ready(Ok(())) => {
                    self.need_flush = false;
                    Poll::Ready(Ok(()))
                }
                result => result,
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.session.wants_write() {
            match self.write_tls_direct(cx) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(_)) => {}
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        if self.state.writeable() {
            self.session.send_close_notify();
            self.state.shutdown_write();
        }

        while self.session.wants_write() {
            match self.write_tls_direct(cx) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(_)) => {}
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        match Pin::new(&mut self.io).poll_shutdown(cx) {
            Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::NotConnected => {
                Poll::Ready(Ok(()))
            }
            result => result,
        }
    }
}

struct SyncReadAdapter<'a, 'b, T> {
    io: &'a mut T,
    cx: &'a mut Context<'b>,
}

impl<T> io::Read for SyncReadAdapter<'_, '_, T>
where
    T: AsyncRead + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read_buf = ReadBuf::new(buf);
        match Pin::new(&mut self.io).poll_read(self.cx, &mut read_buf) {
            Poll::Ready(Ok(())) => Ok(read_buf.filled().len()),
            Poll::Ready(Err(error)) => Err(error),
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

struct SyncWriteAdapter<'a, 'b, T> {
    io: &'a mut T,
    cx: &'a mut Context<'b>,
}

impl<T> io::Write for SyncWriteAdapter<'_, '_, T>
where
    T: AsyncWrite + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match Pin::new(&mut self.io).poll_write(self.cx, buf) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match Pin::new(&mut self.io).poll_flush(self.cx) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}
