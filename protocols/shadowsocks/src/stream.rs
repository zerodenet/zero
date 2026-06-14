use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{
    decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload, derive_download_key, encrypt_tcp_chunk,
    CipherKind, ShadowsocksAccept, ShadowsocksOutboundSession, TCP_CHUNK_SIZE_LEN,
};

enum ReadState {
    Salt {
        buf: Vec<u8>,
        pos: usize,
    },
    /// 2022 outbound: read the response fixed-length header chunk after the
    /// salt (it carries the request salt and the first payload length).
    ResponseHeader2022 {
        buf: Vec<u8>,
        pos: usize,
    },
    /// 2022 outbound: read the first payload chunk whose length came from the
    /// response header. Subsequent chunks use the normal Length/Payload loop.
    FirstPayload2022 {
        expected_len: usize,
        buf: Vec<u8>,
        pos: usize,
    },
    Length {
        buf: Vec<u8>,
        pos: usize,
    },
    Payload {
        expected_len: usize,
        buf: Vec<u8>,
        pos: usize,
    },
    Eof,
}

pub struct ShadowsocksAeadStream<S> {
    inner: S,
    cipher: CipherKind,
    read_key: Option<Vec<u8>>,
    read_password: Option<Vec<u8>>,
    read_nonce: u64,
    read_state: ReadState,
    read_plain: Vec<u8>,
    read_plain_pos: usize,
    write_key: Vec<u8>,
    write_nonce: u64,
    write_buf: Vec<u8>,
    write_pos: usize,
    /// True for 2022 edition streams.
    is_2022: bool,
    /// Inbound: the request salt echoed in the response fixed header.
    /// Outbound: the request salt we sent, verified against the response header.
    request_salt: Vec<u8>,
    /// Inbound 2022: the response salt, emitted with the first response header chunk.
    response_salt: Vec<u8>,
    /// Inbound 2022: true until the first write emits the response header chunk.
    write_response_header_pending: bool,
}

impl<S> ShadowsocksAeadStream<S> {
    #[allow(clippy::too_many_arguments)]
    pub fn inbound(
        inner: S,
        cipher: CipherKind,
        upload_key: Vec<u8>,
        next_upload_nonce: u64,
        download_key: Vec<u8>,
        response_salt: Vec<u8>,
        remaining_payload: Vec<u8>,
        is_2022: bool,
        request_salt: Vec<u8>,
    ) -> Self {
        let is_2022_enabled = is_2022 && cipher.is_blake3();
        // For 2022 the response salt is emitted together with the first
        // response header chunk, so write_buf starts empty and the first
        // write builds salt+header+payload. For legacy, write the salt up
        // front exactly as before.
        let write_buf = if is_2022_enabled {
            Vec::new()
        } else {
            response_salt.clone()
        };
        Self {
            inner,
            cipher,
            read_key: Some(upload_key),
            read_password: None,
            read_nonce: next_upload_nonce,
            read_state: ReadState::Length {
                buf: vec![0_u8; TCP_CHUNK_SIZE_LEN + cipher.tag_len()],
                pos: 0,
            },
            read_plain: remaining_payload,
            read_plain_pos: 0,
            write_key: download_key,
            write_nonce: 0,
            write_buf,
            write_pos: 0,
            is_2022: is_2022_enabled,
            request_salt,
            response_salt,
            write_response_header_pending: is_2022_enabled,
        }
    }

    pub fn outbound(inner: S, session: ShadowsocksOutboundSession, password: Vec<u8>) -> Self {
        let cipher = session.cipher;
        let is_2022 = cipher.is_blake3();
        Self {
            inner,
            cipher,
            read_key: None,
            read_password: Some(password),
            read_nonce: 0,
            read_state: ReadState::Salt {
                buf: vec![0_u8; cipher.salt_len()],
                pos: 0,
            },
            read_plain: Vec::new(),
            read_plain_pos: 0,
            write_key: session.session_key,
            write_nonce: session.next_upload_nonce,
            write_buf: Vec::new(),
            write_pos: 0,
            is_2022,
            request_salt: session.request_salt,
            response_salt: Vec::new(),
            write_response_header_pending: false,
        }
    }

    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl ShadowsocksAccept {
    /// Wrap an accepted inbound TCP stream with Shadowsocks AEAD framing.
    ///
    /// The protocol crate owns the server-to-client response salt and download
    /// key derivation. The runtime only provides the already accepted transport.
    pub fn into_aead_stream<S>(
        self,
        inner: S,
        password: &[u8],
    ) -> Result<ShadowsocksAeadStream<S>, zero_core::Error> {
        let mut response_salt = vec![0_u8; self.cipher.salt_len()];
        use ring::rand::SecureRandom;
        ring::rand::SystemRandom::new()
            .fill(&mut response_salt)
            .map_err(|_| zero_core::Error::Protocol("ss: response salt random failed"))?;
        self.into_aead_stream_with_response_salt(inner, password, response_salt)
    }

    /// Wrap an accepted inbound TCP stream with an explicit response salt.
    ///
    /// This is primarily useful for deterministic protocol tests.
    pub fn into_aead_stream_with_response_salt<S>(
        self,
        inner: S,
        password: &[u8],
        response_salt: Vec<u8>,
    ) -> Result<ShadowsocksAeadStream<S>, zero_core::Error> {
        let download_key = derive_download_key(self.cipher, password, &response_salt)?;
        let is_2022 = self.cipher.is_blake3();
        Ok(ShadowsocksAeadStream::inbound(
            inner,
            self.cipher,
            self.session_key,
            self.next_upload_nonce,
            download_key,
            response_salt,
            self.remaining_payload,
            is_2022,
            self.request_salt,
        ))
    }
}

impl<S> ShadowsocksAeadStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn serve_read_plain(&mut self, buf: &mut ReadBuf<'_>) -> bool {
        if self.read_plain_pos >= self.read_plain.len() {
            self.read_plain.clear();
            self.read_plain_pos = 0;
            return false;
        }

        let available = &self.read_plain[self.read_plain_pos..];
        let n = available.len().min(buf.remaining());
        buf.put_slice(&available[..n]);
        self.read_plain_pos += n;
        true
    }

    fn poll_read_decrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 || self.serve_read_plain(buf) {
            return Poll::Ready(Ok(()));
        }

        loop {
            match &mut self.read_state {
                ReadState::Salt { buf: salt, pos } => {
                    match poll_fill(&mut self.inner, cx, salt, pos, false)? {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            let password = self.read_password.take().ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "shadowsocks read password missing",
                                )
                            })?;
                            self.read_key =
                                Some(derive_download_key(self.cipher, &password, salt).map_err(
                                    |error| io::Error::new(io::ErrorKind::InvalidData, error),
                                )?);
                            if self.is_2022 {
                                // 2022 response: read the fixed-length header
                                // chunk next (carries request salt + first
                                // payload length).
                                let header_len =
                                    crate::shared::ss_2022_response_header_plain_len(salt.len())
                                        + self.cipher.tag_len();
                                self.read_state = ReadState::ResponseHeader2022 {
                                    buf: vec![0_u8; header_len],
                                    pos: 0,
                                };
                            } else {
                                self.read_state = ReadState::Length {
                                    buf: vec![0_u8; TCP_CHUNK_SIZE_LEN + self.cipher.tag_len()],
                                    pos: 0,
                                };
                            }
                        }
                    }
                }
                ReadState::ResponseHeader2022 { buf, pos } => {
                    match poll_fill(&mut self.inner, cx, buf, pos, false)? {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            let key = self.read_key.as_ref().ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "shadowsocks read key missing",
                                )
                            })?;
                            let header_plain = crate::shared::decrypt_tcp_2022_single_chunk(
                                self.cipher,
                                key,
                                &mut self.read_nonce,
                                buf,
                            )
                            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                            let salt_len = self.cipher.salt_len();
                            let (header_type, timestamp, resp_request_salt, length) =
                                crate::shared::parse_2022_response_fixed_header(
                                    &header_plain,
                                    salt_len,
                                )
                                .map_err(|error| {
                                    io::Error::new(io::ErrorKind::InvalidData, error)
                                })?;
                            if header_type != crate::shared::SS_2022_HEADER_TYPE_SERVER_STREAM {
                                return Poll::Ready(Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "ss: 2022 response header bad type",
                                )));
                            }
                            #[cfg(feature = "blake3")]
                            {
                                crate::shared::validate_2022_timestamp(timestamp).map_err(
                                    |error| io::Error::new(io::ErrorKind::InvalidData, error),
                                )?;
                            }
                            // SIP022 3.1.3: the client MUST verify the echoed request salt.
                            if resp_request_salt != self.request_salt {
                                return Poll::Ready(Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "ss: 2022 response request salt mismatch",
                                )));
                            }
                            self.read_state = ReadState::FirstPayload2022 {
                                expected_len: length as usize,
                                buf: vec![0_u8; length as usize + self.cipher.tag_len()],
                                pos: 0,
                            };
                        }
                    }
                }
                ReadState::FirstPayload2022 {
                    expected_len,
                    buf: encrypted,
                    pos,
                } => match poll_fill(&mut self.inner, cx, encrypted, pos, false)? {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        let key = self.read_key.as_ref().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "shadowsocks read key missing",
                            )
                        })?;
                        self.read_plain = crate::shared::decrypt_tcp_2022_single_chunk(
                            self.cipher,
                            key,
                            &mut self.read_nonce,
                            encrypted,
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                        if self.read_plain.len() != *expected_len {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "ss: 2022 first payload length mismatch",
                            )));
                        }
                        self.read_plain_pos = 0;
                        self.read_state = ReadState::Length {
                            buf: vec![0_u8; TCP_CHUNK_SIZE_LEN + self.cipher.tag_len()],
                            pos: 0,
                        };
                        if self.serve_read_plain(buf) {
                            return Poll::Ready(Ok(()));
                        }
                    }
                },
                ReadState::Length {
                    buf: encrypted_len,
                    pos,
                } => match poll_fill(&mut self.inner, cx, encrypted_len, pos, true)? {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        if encrypted_len.is_empty() {
                            self.read_state = ReadState::Eof;
                            return Poll::Ready(Ok(()));
                        }
                        let key = self.read_key.as_ref().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "shadowsocks read key missing",
                            )
                        })?;
                        let expected_len = decrypt_tcp_chunk_length(
                            self.cipher,
                            key,
                            &mut self.read_nonce,
                            encrypted_len,
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                        self.read_state = ReadState::Payload {
                            expected_len,
                            buf: vec![0_u8; expected_len + self.cipher.tag_len()],
                            pos: 0,
                        };
                    }
                },
                ReadState::Payload {
                    expected_len,
                    buf: encrypted_payload,
                    pos,
                } => match poll_fill(&mut self.inner, cx, encrypted_payload, pos, false)? {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(()) => {
                        let key = self.read_key.as_ref().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "shadowsocks read key missing",
                            )
                        })?;
                        self.read_plain = decrypt_tcp_chunk_payload(
                            self.cipher,
                            key,
                            &mut self.read_nonce,
                            *expected_len,
                            encrypted_payload,
                        )
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                        self.read_plain_pos = 0;
                        self.read_state = ReadState::Length {
                            buf: vec![0_u8; TCP_CHUNK_SIZE_LEN + self.cipher.tag_len()],
                            pos: 0,
                        };
                        if self.serve_read_plain(buf) {
                            return Poll::Ready(Ok(()));
                        }
                    }
                },
                ReadState::Eof => return Poll::Ready(Ok(())),
            }
        }
    }

    fn poll_flush_pending(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.write_pos < self.write_buf.len() {
            match Pin::new(&mut self.inner).poll_write(cx, &self.write_buf[self.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "shadowsocks write zero",
                    )));
                }
                Poll::Ready(Ok(n)) => self.write_pos += n,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }
        self.write_buf.clear();
        self.write_pos = 0;
        Poll::Ready(Ok(()))
    }

    fn poll_write_encrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.poll_flush_pending(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Pending => return Poll::Pending,
        }

        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let n = buf.len().min(crate::shared::MAX_TCP_PAYLOAD_SIZE);

        // 2022 inbound: the first write emits the response salt + the
        // fixed-length response header chunk (nonce 0, which doubles as the
        // first length chunk) + the first payload chunk (nonce 1). Body
        // length+payload pairs continue from nonce 2 via encrypt_tcp_chunk.
        if self.is_2022 && self.write_response_header_pending {
            let header_plain = crate::shared::build_2022_response_fixed_header(
                crate::shared::now_unix_seconds(),
                &self.request_salt,
                n as u16,
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let enc_header = crate::shared::encrypt_tcp_2022_single_chunk(
                self.cipher,
                &self.write_key,
                &mut self.write_nonce,
                &header_plain,
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let enc_payload = crate::shared::encrypt_tcp_2022_single_chunk(
                self.cipher,
                &self.write_key,
                &mut self.write_nonce,
                &buf[..n],
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

            self.write_buf.clear();
            self.write_buf.extend_from_slice(&self.response_salt);
            self.write_buf.extend_from_slice(&enc_header);
            self.write_buf.extend_from_slice(&enc_payload);
            self.write_pos = 0;
            self.write_response_header_pending = false;
            return Poll::Ready(Ok(n));
        }

        self.write_buf = encrypt_tcp_chunk(
            self.cipher,
            &self.write_key,
            &mut self.write_nonce,
            &buf[..n],
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.write_pos = 0;
        Poll::Ready(Ok(n))
    }
}

impl<S> AsyncRead for ShadowsocksAeadStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::into_inner(self).poll_read_decrypted(cx, buf)
    }
}

impl<S> AsyncWrite for ShadowsocksAeadStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::into_inner(self).poll_write_encrypted(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);
        match this.poll_flush_pending(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.inner).poll_flush(cx),
            other => other,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);
        match this.poll_flush_pending(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut this.inner).poll_shutdown(cx),
            other => other,
        }
    }
}

fn poll_fill<S>(
    inner: &mut S,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
    pos: &mut usize,
    allow_clean_eof: bool,
) -> io::Result<Poll<()>>
where
    S: AsyncRead + Unpin,
{
    while *pos < buf.len() {
        let mut read_buf = ReadBuf::new(&mut buf[*pos..]);
        match Pin::new(&mut *inner).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) if read_buf.filled().is_empty() => {
                if allow_clean_eof && *pos == 0 {
                    buf.clear();
                    return Ok(Poll::Ready(()));
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "shadowsocks unexpected EOF",
                ));
            }
            Poll::Ready(Ok(())) => *pos += read_buf.filled().len(),
            Poll::Ready(Err(error)) => return Err(error),
            Poll::Pending => return Ok(Poll::Pending),
        }
    }
    Ok(Poll::Ready(()))
}
