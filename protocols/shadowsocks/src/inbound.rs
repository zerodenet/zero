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
