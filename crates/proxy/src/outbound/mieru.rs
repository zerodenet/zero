//! Mieru TCP outbound — encrypted relay stream.
//!
//! Wraps a raw TCP stream with Mieru protocol encryption/decryption,
//! providing an `AsyncRead + AsyncWrite` interface for the proxy relay.
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use mieru::MieruOutbound;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_traits::TcpSessionProtocol;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// Run a socks5 client handshake (no-auth + CONNECT) over an established mieru
/// session stream to bind `target`. After this returns, the stream is a raw
/// bidirectional pipe to the target.
///
/// mieru conveys the proxy target via socks5 inside the encrypted tunnel:
/// mita runs a socks5 server on the decrypted session, so the client must
/// negotiate the target with a CONNECT after the mieru handshake.
pub(crate) async fn socks5_connect<S>(stream: &mut S, target: &Address, port: u16) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // mieru conveys the proxy target via a socks5 request sent DIRECTLY inside
    // the encrypted tunnel — NO greeting/method/auth negotiation (the mieru
    // session itself is the authentication). This matches the upstream mieru
    // client PostDialHandshake: write the request, read the reply, nothing more.
    //
    // CONNECT request.
    let mut req = vec![0x05, 0x01, 0x00];
    match target {
        Address::Ipv4(ip) => {
            req.push(0x01);
            req.extend_from_slice(ip);
        }
        Address::Ipv6(ip) => {
            req.push(0x04);
            req.extend_from_slice(ip);
        }
        Address::Domain(domain) => {
            let b = domain.as_bytes();
            if b.len() > 255 {
                return Err(io::Error::other("mieru socks5: domain too long"));
            }
            req.push(0x03);
            req.push(b.len() as u8);
            req.extend_from_slice(b);
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&req).await?;
    stream.flush().await?;

    // Reply: VER, REP, RSV, ATYP, BND.ADDR, BND.PORT.
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(io::Error::other("mieru socks5: bad reply version"));
    }
    if head[1] != 0x00 {
        return Err(io::Error::other(format!(
            "mieru socks5: connect rejected (rep=0x{:02x})",
            head[1]
        )));
    }
    let bnd_len = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            len[0] as usize
        }
        _ => return Err(io::Error::other("mieru socks5: bad BND address type")),
    };
    let mut bnd_addr = vec![0u8; bnd_len];
    stream.read_exact(&mut bnd_addr).await?;
    let mut bnd_port = [0u8; 2];
    stream.read_exact(&mut bnd_port).await?;
    Ok(())
}

/// A Mieru-encrypted TCP stream that transparently encrypts/decrypts
/// Mieru protocol segments during relay.
pub(crate) struct MieruTcpStream {
    inner: TcpRelayStream,
    outbound: MieruOutbound,
    write_buf: Vec<u8>,
    write_pos: usize,
    raw_read_buf: Vec<u8>,
    /// Buffered decrypted data from the last read.
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl MieruTcpStream {
    pub fn new(inner: TcpRelayStream, outbound: MieruOutbound) -> Self {
        Self {
            inner,
            outbound,
            write_buf: Vec::new(),
            write_pos: 0,
            raw_read_buf: Vec::new(),
            read_buf: Vec::new(),
            read_pos: 0,
        }
    }

    /// Write plaintext → encrypt + send as Mieru data segment.
    fn poll_write_encrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.write_buf.is_empty() {
            self.write_buf = self
                .outbound
                .encrypt_client_data(buf)
                .map_err(|e| io::Error::other(format!("mieru encrypt: {e}")))?;
            self.write_pos = 0;
        }

        while self.write_pos < self.write_buf.len() {
            match Pin::new(&mut self.inner).poll_write(cx, &self.write_buf[self.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mieru write zero",
                    )))
                }
                Poll::Ready(Ok(n)) => self.write_pos += n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        self.write_buf.clear();
        self.write_pos = 0;
        Poll::Ready(Ok(buf.len()))
    }

    /// Read encrypted data → decrypt Mieru segment → return plaintext.
    fn poll_read_decrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Serve buffered data first
        if self.read_pos < self.read_buf.len() {
            let remaining = &self.read_buf[self.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            self.read_pos += n;
            if self.read_pos >= self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        loop {
            match self
                .outbound
                .decrypt_server_data_with_consumed(&self.raw_read_buf)
            {
                Ok((segment, consumed)) => {
                    self.raw_read_buf.drain(..consumed);
                    let payload = segment.payload;
                    if payload.is_empty() {
                        continue;
                    }
                    let n = payload.len().min(buf.remaining());
                    buf.put_slice(&payload[..n]);
                    if n < payload.len() {
                        self.read_buf = payload[n..].to_vec();
                        self.read_pos = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(error) if error == zero_core::Error::Protocol("mieru: need more data") => {
                    let mut scratch = [0u8; 4096];
                    let mut read_buf = ReadBuf::new(&mut scratch);
                    match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let filled = read_buf.filled();
                            if filled.is_empty() {
                                return Poll::Ready(Ok(()));
                            }
                            self.raw_read_buf.extend_from_slice(filled);
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Err(e) => return Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {e}")))),
            }
        }
    }
}

impl AsyncRead for MieruTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::into_inner(self).poll_read_decrypted(cx, buf)
    }
}

impl AsyncWrite for MieruTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::into_inner(self).poll_write_encrypted(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ── TCP outbound connect ──────────────────────────────────────────────

/// Establish a Mieru TCP upstream: dial the server, run the Mieru session
/// handshake, then run a SOCKS5 CONNECT inside the encrypted tunnel to bind
/// the proxy target.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    // Wrap in TcpRelayStream for AsyncSocket compatibility.
    let mut stream = TcpRelayStream::new(socket);

    let outbound =
        <mieru::MieruProtocol as TcpSessionProtocol<mieru::MieruTcpTarget>>::establish_tcp_session(
            &mieru::MieruProtocol,
            &mut stream,
            &mieru::MieruTcpTarget { username, password },
        )
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("mieru handshake: {e}"))))?;

    let mut mieru_stream = MieruTcpStream::new(stream, outbound);
    // Mieru conveys the proxy target via socks5 inside the encrypted tunnel.
    socks5_connect(&mut mieru_stream, &session.target, session.port)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("mieru socks5: {e}"))))?;
    Ok(TcpRelayStream::new(mieru_stream))
}

/// Apply a Mieru session handshake over an existing stream (relay hop).
/// Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let outbound =
        <mieru::MieruProtocol as TcpSessionProtocol<mieru::MieruTcpTarget>>::establish_tcp_session(
            &mieru::MieruProtocol,
            &mut stream,
            &mieru::MieruTcpTarget { username, password },
        )
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("mieru handshake: {e}"))))?;
    let mut mieru_stream = MieruTcpStream::new(stream, outbound);
    socks5_connect(&mut mieru_stream, &session.target, session.port)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("mieru socks5: {e}"))))?;
    Ok(TcpRelayStream::new(mieru_stream))
}
