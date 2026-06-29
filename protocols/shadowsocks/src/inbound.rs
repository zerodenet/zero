// Shadowsocks inbound protocol.

#[cfg(feature = "crypto")]
use alloc::string::String;
#[cfg(all(feature = "crypto", feature = "blake3"))]
use alloc::sync::Arc;
#[cfg(feature = "crypto")]
use alloc::vec::Vec;
#[cfg(feature = "crypto")]
use zero_core::Address;
use zero_core::ProtocolType;
#[cfg(feature = "crypto")]
use zero_core::{Error, Network, Session};
#[cfg(feature = "crypto")]
use zero_traits::{DatagramCodec, UdpDatagramFraming};

/// Shadowsocks inbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShadowsocksInbound;

/// Result of accepting a Shadowsocks TCP connection.
#[cfg(feature = "crypto")]
pub struct ShadowsocksAccept {
    pub session: Session,
    /// Remaining plaintext payload after the target address in the first chunk.
    pub remaining_payload: Vec<u8>,
    /// Derived session key for subsequent AEAD operations.
    pub session_key: Vec<u8>,
    /// Cipher kind for subsequent chunks.
    pub cipher: super::shared::CipherKind,
    /// Next nonce for decrypting client-to-server chunks after the first request chunk.
    pub next_upload_nonce: u64,
    /// For 2022 edition: the client request salt, echoed back in the server
    /// response fixed header. Empty for legacy AEAD.
    pub request_salt: Vec<u8>,
}

/// Protocol-owned validated inbound profile.
///
/// Proxy runtime code keeps this as an opaque profile and delegates TCP/UDP
/// Shadowsocks framing decisions back to the protocol crate.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone)]
pub struct ShadowsocksInboundProfile {
    cipher_name: String,
    cipher: super::shared::CipherKind,
    password: Vec<u8>,
}

/// Listener-scoped Shadowsocks TCP state.
///
/// The proxy keeps this value with its inbound handler and delegates
/// protocol-private replay checks to it.
#[cfg(feature = "crypto")]
#[derive(Clone)]
pub struct ShadowsocksInboundTcpState {
    cipher: super::shared::CipherKind,
    #[cfg(feature = "blake3")]
    replay_pool: Arc<super::shared::ReplaySaltPool>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundTcpState {
    fn new(cipher: super::shared::CipherKind) -> Self {
        Self {
            cipher,
            #[cfg(feature = "blake3")]
            replay_pool: Arc::new(super::shared::ReplaySaltPool::new()),
        }
    }

    pub fn check_accept_replay(&self, accept: &ShadowsocksAccept) -> Result<(), Error> {
        #[cfg(feature = "blake3")]
        {
            if self.cipher.is_blake3() && !accept.request_salt.is_empty() {
                self.replay_pool.check_and_insert(&accept.request_salt)?;
            }
        }
        #[cfg(not(feature = "blake3"))]
        {
            let _ = accept;
        }
        Ok(())
    }
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundProfile {
    pub fn from_config(cipher_name: &str, password: &str) -> Result<Self, Error> {
        let cipher = super::shared::CipherKind::from_str(cipher_name)
            .ok_or(Error::Protocol("ss: unknown inbound cipher"))?;
        Ok(Self {
            cipher_name: String::from(cipher_name),
            cipher,
            password: password.as_bytes().to_vec(),
        })
    }

    pub fn cipher_name(&self) -> &str {
        &self.cipher_name
    }

    pub fn principal_key(&self) -> String {
        String::from_utf8_lossy(&self.password).to_string()
    }

    pub fn is_2022(&self) -> bool {
        self.cipher.is_blake3()
    }

    pub fn tcp_state(&self) -> ShadowsocksInboundTcpState {
        ShadowsocksInboundTcpState::new(self.cipher)
    }

    pub fn udp_codec(&self) -> ShadowsocksInboundUdpCodec {
        ShadowsocksInboundUdpCodec::new(self.cipher, &self.password)
    }

    pub fn udp_session(&self) -> ShadowsocksInboundUdpSession {
        ShadowsocksInboundUdpSession::new(self.udp_codec())
    }

    pub async fn accept_request<S: zero_traits::AsyncSocket>(
        &self,
        inbound: &ShadowsocksInbound,
        stream: &mut S,
    ) -> Result<ShadowsocksAccept, Error> {
        inbound
            .accept_request(stream, self.cipher, &self.password)
            .await
    }

    pub fn into_aead_stream<S>(
        &self,
        accept: ShadowsocksAccept,
        inner: S,
    ) -> Result<super::stream::ShadowsocksAeadStream<S>, Error> {
        accept.into_aead_stream(inner, &self.password)
    }
}

/// Decoded Shadowsocks inbound UDP request.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    /// Flow isolation id for SIP022/2022 UDP sessions. `None` for legacy AEAD.
    client_session_id: Option<u64>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpPacket {
    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn client_session_id(&self) -> Option<u64> {
        self.client_session_id
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }

    pub fn into_dispatch_parts(self) -> ShadowsocksInboundUdpDispatchParts {
        let (target, port, payload, client_session_id) = self.into_parts();
        ShadowsocksInboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id,
        }
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpDispatchParts {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
    pub client_session_id: Option<u64>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpDispatchParts {
    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpResponse {
    datagram: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponse {
    pub fn datagram(&self) -> &[u8] {
        &self.datagram
    }

    pub fn into_datagram(self) -> Vec<u8> {
        self.datagram
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpResponseTarget {
    client_session_id: Option<u64>,
    target: Address,
    port: u16,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponseTarget {
    pub fn new(client_session_id: Option<u64>, target: Address, port: u16) -> Self {
        Self {
            client_session_id,
            target,
            port,
        }
    }

    pub fn from_parts(client_session_id: Option<u64>, target: &Address, port: u16) -> Self {
        Self::new(client_session_id, target.clone(), port)
    }
}

/// Protocol-owned codec/state for Shadowsocks inbound UDP.
///
/// Runtime code owns socket I/O and routing, while this type owns
/// Shadowsocks UDP decoding, SIP022 replay protection, and response encoding.
#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpCodec {
    cipher: super::shared::CipherKind,
    password: Vec<u8>,
    #[cfg(feature = "blake3")]
    replay_windows: std::collections::HashMap<u64, super::shared::ReplayWindow>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpCodec {
    pub fn new(cipher: super::shared::CipherKind, password: &[u8]) -> Self {
        Self {
            cipher,
            password: password.to_vec(),
            #[cfg(feature = "blake3")]
            replay_windows: std::collections::HashMap::new(),
        }
    }

    pub fn decode_request(
        &mut self,
        datagram: &[u8],
    ) -> Result<ShadowsocksInboundUdpPacket, Error> {
        if self.cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                let (target, port, payload, client_session_id, packet_id) =
                    super::shared::decode_udp_datagram_2022_session(
                        self.cipher,
                        &self.password,
                        datagram,
                    )?;
                if !self
                    .replay_windows
                    .entry(client_session_id)
                    .or_default()
                    .check_and_update(packet_id)
                {
                    return Err(Error::Protocol("ss: udp replay rejected"));
                }
                return Ok(ShadowsocksInboundUdpPacket {
                    target,
                    port,
                    payload,
                    client_session_id: Some(client_session_id),
                });
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Protocol(
                "ss: 2022 udp decode requires `blake3` feature",
            ));
        }

        let codec = super::outbound::ShadowsocksDatagramCodec {
            cipher: self.cipher,
            password: self.password.clone(),
        };
        let (target, port, payload) = codec
            .decode(datagram)
            .ok_or(Error::Protocol("ss: udp datagram decode failed"))?;
        Ok(ShadowsocksInboundUdpPacket {
            target,
            port,
            payload,
            client_session_id: None,
        })
    }

    pub fn encode_response(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        if self.cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                return super::shared::encode_udp_response_2022(
                    self.cipher,
                    &self.password,
                    client_session_id.unwrap_or(0),
                    target,
                    port,
                    payload,
                );
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Protocol(
                "ss: 2022 udp encode requires `blake3` feature",
            ));
        }

        <super::outbound::ShadowsocksOutbound as UdpDatagramFraming<
            super::outbound::ShadowsocksUdpPacketTarget<'_>,
            super::outbound::ShadowsocksUdpDecodeContext<'_>,
        >>::encode_udp_datagram(
            &super::outbound::ShadowsocksOutbound,
            &super::outbound::ShadowsocksUdpPacketTarget {
                target,
                port,
                payload,
                cipher: self.cipher,
                password: &self.password,
            },
        )
    }

    pub fn encode_response_to_client(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        Ok(ShadowsocksInboundUdpResponse {
            datagram: self.encode_response(client_session_id, target, port, payload)?,
        })
    }

    pub fn response_frame(
        &self,
        target: &ShadowsocksInboundUdpResponseTarget,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.encode_response_to_client(
            target.client_session_id,
            &target.target,
            target.port,
            payload,
        )
    }
}

#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpSession {
    codec: ShadowsocksInboundUdpCodec,
    proxy_sessions: std::collections::HashMap<u64, Option<u64>>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpSession {
    pub fn new(codec: ShadowsocksInboundUdpCodec) -> Self {
        Self {
            codec,
            proxy_sessions: std::collections::HashMap::new(),
        }
    }

    pub fn decode_request(
        &mut self,
        datagram: &[u8],
    ) -> Result<ShadowsocksInboundUdpPacket, Error> {
        self.codec.decode_request(datagram)
    }

    pub fn encode_response_to_client(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.codec
            .encode_response_to_client(client_session_id, target, port, payload)
    }

    pub fn response_frame(
        &self,
        target: &ShadowsocksInboundUdpResponseTarget,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.codec.response_frame(target, payload)
    }

    pub fn response_frame_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        let response_target =
            self.response_target_for_proxy_session(proxy_session_id, target, port);
        self.response_frame(&response_target, payload)
    }

    pub fn response_datagram_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponseDatagram, Error> {
        let datagram =
            self.response_frame_for_proxy_session(proxy_session_id, target, port, payload)?;
        Ok(ShadowsocksInboundUdpResponseDatagram {
            datagram: datagram.into_datagram(),
        })
    }

    pub async fn send_response_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
        client: std::net::SocketAddr,
    ) -> Result<usize, Error> {
        let datagram =
            self.response_datagram_for_proxy_session(proxy_session_id, target, port, payload)?;
        socket
            .send_to(datagram.as_slice(), client)
            .await
            .map_err(|_| Error::Io("failed to send Shadowsocks UDP response"))
    }

    pub fn record_proxy_session(&mut self, proxy_session_id: u64, client_session_id: Option<u64>) {
        if client_session_id.is_some() {
            self.proxy_sessions
                .insert(proxy_session_id, client_session_id);
        }
    }

    pub fn response_target_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
    ) -> ShadowsocksInboundUdpResponseTarget {
        ShadowsocksInboundUdpResponseTarget::from_parts(
            self.proxy_sessions
                .get(&proxy_session_id)
                .copied()
                .flatten(),
            target,
            port,
        )
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug)]
pub struct ShadowsocksInboundUdpResponseDatagram {
    datagram: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponseDatagram {
    pub fn len(&self) -> usize {
        self.datagram.len()
    }

    pub fn is_empty(&self) -> bool {
        self.datagram.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.datagram
    }

    pub fn into_datagram(self) -> Vec<u8> {
        self.datagram
    }
}

impl ShadowsocksInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Shadowsocks
    }

    /// Decrypt the initial stream payload, extract target address,
    /// and return session key + remaining payload for relay.
    #[cfg(feature = "crypto")]
    pub async fn accept_request<S: zero_traits::AsyncSocket>(
        &self,
        stream: &mut S,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksAccept, Error> {
        if cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                return self.accept_request_2022(stream, cipher, password).await;
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Protocol(
                "ss: 2022 tcp accept requires `blake3` feature",
            ));
        }
        self.accept_request_legacy(stream, cipher, password).await
    }

    /// Legacy AEAD accept: read salt + one length/payload chunk, extract target.
    #[cfg(feature = "crypto")]
    async fn accept_request_legacy<S: zero_traits::AsyncSocket>(
        &self,
        stream: &mut S,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksAccept, Error> {
        use super::shared::{derive_session_key, parse_target_data, read_exact, read_tcp_chunk};

        let salt_len = cipher.salt_len();

        // Read salt
        let mut salt = vec![0u8; salt_len];
        read_exact(stream, &mut salt).await?;

        let key = derive_session_key(cipher, password, &salt)?;

        let mut nonce = 0;
        let plain = read_tcp_chunk(stream, cipher, &key, &mut nonce).await?;

        // Parse target from plaintext
        let (target, port, payload_offset) = parse_target_data(&plain)?;
        let remaining_payload = plain[payload_offset..].to_vec();

        let session = Session::new(0, target, port, Network::Tcp, ProtocolType::Shadowsocks);

        Ok(ShadowsocksAccept {
            session,
            remaining_payload,
            session_key: key,
            cipher,
            next_upload_nonce: nonce,
            request_salt: salt,
        })
    }

    /// 2022 edition (SIP022) accept: read salt + fixed-header chunk (nonce 0)
    /// + variable-header chunk (nonce 1). Body chunks follow from nonce 2.
    ///
    /// Implements SIP022 3.1.3 detection prevention: the salt + fixed-length
    /// header are read in a single `read()` call, and on any handshake failure
    /// the stream is drained before returning so the subsequent close sends FIN
    /// rather than RST (hiding how many bytes the server consumed).
    #[cfg(all(feature = "crypto", feature = "blake3"))]
    async fn accept_request_2022<S: zero_traits::AsyncSocket>(
        &self,
        stream: &mut S,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksAccept, Error> {
        match self
            .accept_request_2022_probe(stream, cipher, password)
            .await
        {
            Ok(accept) => Ok(accept),
            Err(error) => {
                // Drain to hide byte consumption from active probers.
                drain_stream(stream, SS_2022_DRAIN_CAP).await;
                Err(error)
            }
        }
    }

    /// Single-read + validate the 2022 request, without drain-on-error. The
    /// caller ([`accept_request_2022`]) drains on failure.
    #[cfg(all(feature = "crypto", feature = "blake3"))]
    async fn accept_request_2022_probe<S: zero_traits::AsyncSocket>(
        &self,
        stream: &mut S,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksAccept, Error> {
        use super::shared::{
            decrypt_tcp_2022_single_chunk, derive_session_key, parse_2022_request_fixed_header,
            parse_2022_request_var_header, validate_2022_timestamp,
            SS_2022_HEADER_TYPE_CLIENT_STREAM, SS_2022_REQUEST_FIXED_HEADER_LEN,
        };

        let salt_len = cipher.salt_len();
        let fixed_size = SS_2022_REQUEST_FIXED_HEADER_LEN + cipher.tag_len();

        // SIP022 3.1.3: exactly ONE read for salt + fixed-length header. A
        // short read means a probe (or a fragmenting path); reject it.
        let mut head = vec![0u8; salt_len + fixed_size];
        let n = stream
            .read(&mut head)
            .await
            .map_err(|_| Error::Io("ss: 2022 request read failed"))?;
        if n < salt_len + fixed_size {
            return Err(Error::Protocol("ss: 2022 request header too short"));
        }

        let key = derive_session_key(cipher, password, &head[..salt_len])?;
        let mut nonce = 0u64;
        let fixed_plain = decrypt_tcp_2022_single_chunk(
            cipher,
            &key,
            &mut nonce,
            &head[salt_len..salt_len + fixed_size],
        )?;
        let (header_type, timestamp, var_len) = parse_2022_request_fixed_header(&fixed_plain)?;
        if header_type != SS_2022_HEADER_TYPE_CLIENT_STREAM {
            return Err(Error::Protocol("ss: 2022 request header bad type"));
        }
        validate_2022_timestamp(timestamp)?;

        // Variable-length header: one read of its AEAD chunk.
        let var_len = var_len as usize;
        let var_size = var_len + cipher.tag_len();
        let mut enc_var = vec![0u8; var_size];
        let vn = stream
            .read(&mut enc_var)
            .await
            .map_err(|_| Error::Io("ss: 2022 variable header read failed"))?;
        if vn < var_size {
            return Err(Error::Protocol("ss: 2022 variable header too short"));
        }
        let var_plain =
            decrypt_tcp_2022_single_chunk(cipher, &key, &mut nonce, &enc_var[..var_size])?;
        if var_plain.len() != var_len {
            return Err(Error::Protocol("ss: 2022 variable header length mismatch"));
        }
        let (target, port, initial_payload) = parse_2022_request_var_header(&var_plain)?;

        let session = Session::new(0, target, port, Network::Tcp, ProtocolType::Shadowsocks);

        Ok(ShadowsocksAccept {
            session,
            remaining_payload: initial_payload,
            session_key: key,
            cipher,
            next_upload_nonce: nonce,
            request_salt: head[..salt_len].to_vec(),
        })
    }

    /// Encrypt a plaintext chunk for the server-to-client direction.
    #[cfg(feature = "crypto")]
    pub fn encrypt_chunk(
        cipher: super::shared::CipherKind,
        key: &[u8],
        nonce_counter: &mut u64,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        super::shared::encrypt_tcp_chunk(cipher, key, nonce_counter, data)
    }

    /// Decrypt a ciphertext chunk for the client-to-server direction.
    #[cfg(feature = "crypto")]
    pub fn decrypt_chunk(
        cipher: super::shared::CipherKind,
        key: &[u8],
        nonce_counter: &mut u64,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let length_size = super::shared::TCP_CHUNK_SIZE_LEN + cipher.tag_len();
        if data.len() < length_size {
            return Err(Error::Protocol("ss: chunk too short"));
        }
        let payload_len = super::shared::decrypt_tcp_chunk_length(
            cipher,
            key,
            nonce_counter,
            &data[..length_size],
        )?;
        super::shared::decrypt_tcp_chunk_payload(
            cipher,
            key,
            nonce_counter,
            payload_len,
            &data[length_size..],
        )
    }
}

/// SIP022 3.1.3 detection-prevention drain cap (bytes). Bounds the drain so a
/// malicious peer cannot hold the connection open indefinitely; typical active
/// probes send far fewer bytes than this.
const SS_2022_DRAIN_CAP: usize = 1 << 20; // 1 MiB
/// Hard wall on how long a failed-handshake drain may block. A peer that sends
/// a short probe and then holds the connection open would otherwise pin a task
/// until the byte cap is reached; this keeps the anti-probe drain bounded.
const SS_2022_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Drain up to `cap` bytes (or `timeout`, or EOF) from `stream`, discarding
/// them. Used after a failed 2022 handshake so closing the connection sends FIN
/// (empty receive buffer) instead of RST, hiding how many bytes the server
/// consumed. Bounded by both a byte cap and a wall-clock timeout so a peer
/// cannot pin the task by keeping the connection open after a short probe.
#[cfg(all(feature = "crypto", feature = "blake3"))]
async fn drain_stream<S: zero_traits::AsyncSocket>(stream: &mut S, cap: usize) {
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    let mut deadline_reached = false;
    while total < cap && !deadline_reached {
        // Bound each read so a silent peer cannot block forever.
        match tokio::time::timeout(SS_2022_DRAIN_TIMEOUT, stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => total += n,
            Ok(Err(_)) => break,
            Err(_) => {
                deadline_reached = true;
            }
        }
    }
}
