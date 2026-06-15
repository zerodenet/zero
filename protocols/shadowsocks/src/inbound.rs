// Shadowsocks inbound protocol.

#[cfg(feature = "crypto")]
use alloc::vec::Vec;
use zero_core::ProtocolType;
#[cfg(feature = "crypto")]
use zero_core::{Error, Network, Session};

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

/// Drain up to `cap` bytes from `stream`, discarding them. Used after a failed
/// 2022 handshake so closing the connection sends FIN (empty receive buffer)
/// instead of RST, hiding how many bytes the server consumed.
#[cfg(all(feature = "crypto", feature = "blake3"))]
async fn drain_stream<S: zero_traits::AsyncSocket>(stream: &mut S, cap: usize) {
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    while total < cap {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(_) => break,
        }
    }
}
