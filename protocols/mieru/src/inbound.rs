// Mieru protocol inbound handler — inbound.rs

use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_core::{Address, Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::crypto::{try_derive_keys, MieruCipher};
use crate::metadata::{
    DataMetadata, SessionMetadata, DATA_SERVER_TO_CLIENT, METADATA_LEN, OPEN_SESSION_REQUEST,
    OPEN_SESSION_RESPONSE,
};
use crate::segment::{build_data_segment, build_session_segment, parse_segment, Segment};
use crate::session::MieruSession;

/// Mieru inbound handler.
#[derive(Debug, Default, Clone)]
pub struct MieruInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruInboundProfile {
    users: Vec<(String, String)>,
}

impl MieruInboundProfile {
    pub fn from_config(users: Vec<(String, String)>) -> Self {
        Self { users }
    }

    pub fn from_config_parts<I>(users: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self::from_config(users.into_iter().collect())
    }

    pub fn from_config_users<I, U>(users: I) -> Self
    where
        I: IntoIterator<Item = U>,
        U: IntoMieruInboundUserConfig,
    {
        Self::from_config_parts(users.into_iter().map(U::into_mieru_inbound_user_config))
    }

    pub fn inbound_auth(&self) -> SessionAuth {
        MieruInbound.inbound_auth()
    }

    pub async fn accept_request<S: AsyncSocket>(
        &self,
        inbound: &MieruInbound,
        stream: &mut S,
    ) -> Result<MieruAccept, Error> {
        inbound.accept_request(stream, &self.users).await
    }
}

pub trait IntoMieruInboundUserConfig {
    fn into_mieru_inbound_user_config(self) -> (String, String);
}

impl IntoMieruInboundUserConfig for (String, String) {
    fn into_mieru_inbound_user_config(self) -> (String, String) {
        self
    }
}

impl IntoMieruInboundUserConfig for (&str, &str) {
    fn into_mieru_inbound_user_config(self) -> (String, String) {
        (self.0.to_owned(), self.1.to_owned())
    }
}

/// Result of accepting a mieru TCP connection.
///
/// The mieru session is target-agnostic: it is an encrypted tunnel. The proxy
/// target is conveyed by a socks5 request that the client sends inside the
/// tunnel after the handshake, so the caller must read that request over the
/// decrypted stream to obtain the target (mirroring the upstream mieru model).
pub struct MieruAccept {
    mieru_session: MieruSession,
    client_cipher: MieruCipher,
    server_cipher: MieruCipher,
    /// Bytes already decrypted from the first segment beyond its metadata
    /// (usually empty for socks5-in-tunnel clients).
    remaining_payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MieruInboundSessionKind {
    Tcp,
    Udp,
}

pub fn classify_inbound_session(session: &Session) -> MieruInboundSessionKind {
    match session.network {
        Network::Udp => MieruInboundSessionKind::Udp,
        Network::Tcp => MieruInboundSessionKind::Tcp,
    }
}

fn segment_wire_len(segment: &Segment, has_nonce: bool) -> usize {
    let nonce_len = if has_nonce { 24 } else { 0 };
    let meta_len = METADATA_LEN + 16;
    if let Some(meta) = segment.data_meta.as_ref() {
        nonce_len
            + meta_len
            + meta.prefix_length as usize
            + meta.payload_length as usize
            + if meta.payload_length > 0 { 16 } else { 0 }
            + meta.suffix_length as usize
    } else if let Some(meta) = segment.session_meta.as_ref() {
        nonce_len
            + meta_len
            + meta.payload_length as usize
            + if meta.payload_length > 0 { 16 } else { 0 }
    } else {
        nonce_len + meta_len
    }
}

/// Mieru inbound data-phase codec.
///
/// This type owns the protocol state for decrypting client-to-server data and
/// encrypting server-to-client data after the inbound handshake. Runtime code
/// should wrap it in an I/O adapter instead of touching ciphers or segment
/// metadata directly.
pub struct MieruInboundDataCodec {
    mieru_session: MieruSession,
    client_cipher: MieruCipher,
    server_cipher: MieruCipher,
    c2s_nonce_recv: bool,
    s2c_nonce_sent: bool,
}

impl MieruInboundDataCodec {
    pub fn new(accept: MieruAccept) -> (Self, Vec<u8>) {
        (
            Self {
                mieru_session: accept.mieru_session,
                client_cipher: accept.client_cipher,
                server_cipher: accept.server_cipher,
                c2s_nonce_recv: true,
                s2c_nonce_sent: true,
            },
            accept.remaining_payload,
        )
    }

    pub fn decrypt_client_data_with_consumed(
        &mut self,
        data: &[u8],
    ) -> Result<(Segment, usize), Error> {
        let include_nonce = !self.c2s_nonce_recv;
        let mut client_cipher = self.client_cipher.clone();
        let (segment, consumed) = parse_segment(data, &mut client_cipher, include_nonce, false)?;
        self.client_cipher = client_cipher;
        self.c2s_nonce_recv = true;
        let consumed = consumed.max(segment_wire_len(&segment, include_nonce));
        Ok((segment, consumed))
    }

    pub fn encrypt_server_data(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let metadata = DataMetadata {
            protocol_type: DATA_SERVER_TO_CLIENT,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: self.mieru_session.session_id,
            sequence_number: self.mieru_session.next_send_seq(),
            unack_sequence: 0,
            window_size: 1024,
            fragment_number: 0,
            prefix_length: 0,
            payload_length: data.len() as u16,
            suffix_length: 0,
        };
        let include_nonce = !self.s2c_nonce_sent;
        let segment = build_data_segment(&metadata, data, &mut self.server_cipher, include_nonce)?;
        self.s2c_nonce_sent = true;
        Ok(segment)
    }
}

/// Async stream for the Mieru inbound data phase.
///
/// The stream owns Mieru's data-phase encryption/decryption buffers and exposes
/// plain tunnel bytes to the caller.
pub struct MieruInboundStream<S> {
    inner: S,
    codec: MieruInboundDataCodec,
    read_buf: Vec<u8>,
    read_pos: usize,
    raw_read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    write_pos: usize,
    write_plain_len: usize,
}

impl<S> MieruInboundStream<S> {
    pub fn new(inner: S, accept: MieruAccept) -> Self {
        let (codec, read_buf) = MieruInboundDataCodec::new(accept);
        Self {
            inner,
            codec,
            read_buf,
            read_pos: 0,
            raw_read_buf: Vec::new(),
            write_buf: Vec::new(),
            write_pos: 0,
            write_plain_len: 0,
        }
    }

    pub fn into_inner(self) -> S {
        self.inner
    }

    pub async fn accept_tunneled_socks5_session(&mut self) -> Result<Session, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let request = read_tunneled_socks5_request(self).await?;
        write_tunneled_socks5_success(self).await?;
        Ok(request.into_session())
    }
}

struct MieruTunneledSocks5Request {
    target: Address,
    port: u16,
    network: Network,
}

impl MieruTunneledSocks5Request {
    fn into_session(self) -> Session {
        Session::new(0, self.target, self.port, self.network, ProtocolType::Mieru)
    }
}

async fn read_tunneled_socks5_request<S>(
    stream: &mut S,
) -> Result<MieruTunneledSocks5Request, Error>
where
    S: AsyncRead + Unpin,
{
    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .await
        .map_err(|_| Error::Io("mieru socks5: read request header"))?;

    if head[0] != 0x05 {
        return Err(Error::Protocol("mieru socks5: bad request version"));
    }

    let cmd = head[1];
    let target = read_tunneled_socks5_address(stream, head[3]).await?;

    let mut port_bytes = [0u8; 2];
    stream
        .read_exact(&mut port_bytes)
        .await
        .map_err(|_| Error::Io("mieru socks5: read request port"))?;
    let port = u16::from_be_bytes(port_bytes);

    let network = match cmd {
        0x01 => Network::Tcp,
        0x03 => Network::Udp,
        _ => return Err(Error::Unsupported("mieru socks5: unsupported command")),
    };

    Ok(MieruTunneledSocks5Request {
        target,
        port,
        network,
    })
}

async fn read_tunneled_socks5_address<S>(stream: &mut S, atyp: u8) -> Result<Address, Error>
where
    S: AsyncRead + Unpin,
{
    match atyp {
        0x01 => {
            let mut ip = [0u8; 4];
            stream
                .read_exact(&mut ip)
                .await
                .map_err(|_| Error::Io("mieru socks5: read ipv4 address"))?;
            Ok(Address::Ipv4(ip))
        }
        0x04 => {
            let mut ip = [0u8; 16];
            stream
                .read_exact(&mut ip)
                .await
                .map_err(|_| Error::Io("mieru socks5: read ipv6 address"))?;
            Ok(Address::Ipv6(ip))
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream
                .read_exact(&mut len)
                .await
                .map_err(|_| Error::Io("mieru socks5: read domain length"))?;
            let mut domain = vec![0u8; len[0] as usize];
            stream
                .read_exact(&mut domain)
                .await
                .map_err(|_| Error::Io("mieru socks5: read domain"))?;
            let domain = String::from_utf8(domain)
                .map_err(|_| Error::Protocol("mieru socks5: invalid domain"))?;
            Ok(Address::Domain(domain))
        }
        _ => Err(Error::Protocol("mieru socks5: bad address type")),
    }
}

async fn write_tunneled_socks5_success<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncWrite + Unpin,
{
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|_| Error::Io("mieru socks5: write success"))?;
    stream
        .flush()
        .await
        .map_err(|_| Error::Io("mieru socks5: flush success"))?;
    Ok(())
}

impl<S> AsyncRead for MieruInboundStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);

        if this.read_pos < this.read_buf.len() {
            let remaining = &this.read_buf[this.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            this.read_pos += n;
            if this.read_pos >= this.read_buf.len() {
                this.read_buf.clear();
                this.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        loop {
            match this
                .codec
                .decrypt_client_data_with_consumed(&this.raw_read_buf)
            {
                Ok((segment, consumed)) => {
                    this.raw_read_buf.drain(..consumed);

                    let payload = segment.payload;
                    if payload.is_empty() {
                        continue;
                    }

                    let n = payload.len().min(buf.remaining());
                    buf.put_slice(&payload[..n]);
                    if n < payload.len() {
                        this.read_buf = payload[n..].to_vec();
                        this.read_pos = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(Error::Protocol("mieru: need more data")) => {}
                Err(error) => {
                    return Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {error}"))));
                }
            }

            let before = this.raw_read_buf.len();
            this.raw_read_buf.resize(before + 8192, 0);
            let mut read_buf = ReadBuf::new(&mut this.raw_read_buf[before..]);
            match Pin::new(&mut this.inner).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let filled = read_buf.filled().len();
                    this.raw_read_buf.truncate(before + filled);
                    if filled == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(error)) => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Ready(Err(error));
                }
                Poll::Pending => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<S> AsyncWrite for MieruInboundStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = Pin::into_inner(self);

        if this.write_buf.is_empty() {
            match this.codec.encrypt_server_data(buf) {
                Ok(segment) => {
                    this.write_buf = segment;
                    this.write_pos = 0;
                    this.write_plain_len = buf.len();
                }
                Err(_) => return Poll::Ready(Err(io::Error::other("mieru encrypt failed"))),
            }
        }

        while this.write_pos < this.write_buf.len() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf[this.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mieru write zero",
                    )));
                }
                Poll::Ready(Ok(n)) => {
                    this.write_pos += n;
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        let written = this.write_plain_len;
        this.write_buf.clear();
        this.write_pos = 0;
        this.write_plain_len = 0;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl MieruInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Mieru
    }

    pub fn inbound_auth(&self) -> SessionAuth {
        let mut auth = SessionAuth::new("mieru");
        auth.principal_key = Some("mieru".to_owned());
        auth
    }

    pub fn udp_session(&self) -> crate::udp::MieruInboundUdpSession {
        crate::udp::MieruInboundUdpSession::new()
    }

    /// Accept a mieru TCP connection — perform the mieru handshake only.
    ///
    /// Establishes the encrypted session and replies with openSessionResponse.
    /// The proxy target is NOT known here; the caller reads a socks5 request
    /// over the decrypted stream to obtain it.
    pub async fn accept_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[(String, String)],
    ) -> Result<MieruAccept, Error> {
        // Read first segment: nonce(24) + encrypted_meta(32) + tag(16) = 72 bytes.
        // Upstream mieru (and Zero's outbound) emit no leading padding0, so the
        // nonce is at offset 0.
        const SEGMENT_CORE: usize = 24 + 32 + 16;
        let mut first = vec![0u8; SEGMENT_CORE];
        read_exact(stream, &mut first, SEGMENT_CORE).await?;

        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Protocol("mieru: system time error"))?
            .as_secs();

        // Try each user's key to decrypt the openSessionRequest metadata.
        let mut matched: Option<(MieruCipher, MieruCipher, SessionMetadata)> = None;

        for (username, password) in users {
            let keys = try_derive_keys(username, password, unix_now);
            for key in &keys {
                let mut c = MieruCipher::new(key);
                if let Ok(pt) = c.decrypt(true, &first) {
                    if pt.len() >= METADATA_LEN {
                        let meta = SessionMetadata::decode(&pt[..METADATA_LEN]);
                        if meta.protocol_type == OPEN_SESSION_REQUEST {
                            matched = Some((c, MieruCipher::new(key), meta));
                            break;
                        }
                    }
                }
            }
            if matched.is_some() {
                break;
            }
        }

        let (mut client_cipher, mut server_cipher, open_req) =
            matched.ok_or(Error::Protocol("mieru: no valid user key found"))?;

        // socks5-in-tunnel clients send no target in openSessionRequest. Consume
        // any declared payload defensively; the target arrives via a socks5
        // request in the data phase, read by the proxy handler.
        let remaining_payload = if open_req.payload_length > 0 {
            let plen = open_req.payload_length as usize;
            let mut payload_ct = vec![0u8; plen + 16]; // ciphertext + tag
            read_exact(stream, &mut payload_ct, plen + 16).await?;
            client_cipher.decrypt(false, &payload_ct)?
        } else {
            Vec::new()
        };

        // Send openSessionResponse.
        let session = MieruSession::with_id(open_req.session_id);
        let resp_meta = SessionMetadata {
            protocol_type: OPEN_SESSION_RESPONSE,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: open_req.session_id,
            sequence_number: 0,
            status_code: 0,
            payload_length: 0,
            suffix_length: 0,
        };
        let resp_seg = build_session_segment(&resp_meta, &[], &mut server_cipher, true)?;
        stream
            .write_all(&resp_seg)
            .await
            .map_err(|_| Error::Io("mieru: write response"))?;

        Ok(MieruAccept {
            mieru_session: session,
            client_cipher,
            server_cipher,
            remaining_payload,
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

async fn read_exact<S: AsyncSocket>(
    stream: &mut S,
    buf: &mut [u8],
    len: usize,
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < len {
        let n = stream
            .read(&mut buf[offset..len])
            .await
            .map_err(|_| Error::Io("mieru read"))?;
        if n == 0 {
            return Err(Error::Protocol("mieru: connection closed"));
        }
        offset += n;
    }
    Ok(())
}
