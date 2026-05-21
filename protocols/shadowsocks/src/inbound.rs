// Shadowsocks inbound protocol — inbound.rs

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
        use super::shared::{aead_decrypt, parse_target_data, read_exact};

        let key_len = cipher.key_len();
        let salt_len = cipher.salt_len();

        // Read salt
        let mut salt = vec![0u8; salt_len];
        read_exact(stream, &mut salt).await?;

        // Derive key (SHA1 for legacy, Blake3 for 2022)
        let key = if cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                super::shared::derive_key_blake3(password, &salt, key_len)?
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Unsupported("ss: blake3 feature not enabled"));
        } else {
            super::shared::derive_key(password, &salt, key_len)?
        };

        // Read first chunk (2-byte length + encrypted data)
        let mut len_buf = [0u8; 2];
        read_exact(stream, &mut len_buf).await?;
        let chunk_len = u16::from_be_bytes(len_buf) as usize;
        if chunk_len < cipher.tag_len() {
            return Err(Error::Protocol("ss: chunk too short"));
        }

        let mut chunk = vec![0u8; chunk_len];
        read_exact(stream, &mut chunk).await?;

        // Decrypt: nonce is zero for first chunk
        let nonce = [0u8; 12];
        let plain = aead_decrypt(cipher, &key, &nonce, &chunk)?;

        // Parse target from plaintext
        let (target, port, payload_offset) = parse_target_data(&plain)?;
        let remaining_payload = plain[payload_offset..].to_vec();

        let session = Session::new(0, target, port, Network::Tcp, ProtocolType::Shadowsocks);

        Ok(ShadowsocksAccept {
            session,
            remaining_payload,
            session_key: key,
            cipher,
        })
    }

    /// Encrypt a plaintext chunk for server→client direction.
    #[cfg(feature = "crypto")]
    pub fn encrypt_chunk(
        cipher: super::shared::CipherKind,
        key: &[u8],
        nonce_counter: &mut u64,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&nonce_counter.to_be_bytes());
        *nonce_counter += 1;
        super::shared::aead_encrypt(cipher, key, &nonce, data)
    }

    /// Decrypt a ciphertext chunk for client→server direction.
    #[cfg(feature = "crypto")]
    pub fn decrypt_chunk(
        cipher: super::shared::CipherKind,
        key: &[u8],
        nonce_counter: &mut u64,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&nonce_counter.to_be_bytes());
        *nonce_counter += 1;
        super::shared::aead_decrypt(cipher, key, &nonce, data)
    }
}
