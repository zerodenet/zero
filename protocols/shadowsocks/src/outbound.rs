// Shadowsocks outbound protocol — outbound.rs

use zero_core::ProtocolType;
#[cfg(feature = "crypto")]
use zero_core::{Error, Session};
#[cfg(feature = "crypto")]
use zero_traits::AsyncSocket;

/// Shadowsocks outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShadowsocksOutbound;

#[cfg(feature = "crypto")]
pub struct ShadowsocksOutboundSession {
    pub session_key: Vec<u8>,
    pub next_upload_nonce: u64,
    pub cipher: super::shared::CipherKind,
}

impl ShadowsocksOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Shadowsocks
    }

    /// Write the initial encrypted stream payload containing target address.
    #[cfg(feature = "crypto")]
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksOutboundSession, Error> {
        use super::shared::{build_target_data, encrypt_tcp_chunk};

        let key_len = cipher.key_len();
        let salt_len = cipher.salt_len();

        // Generate random salt
        let mut salt = vec![0u8; salt_len];
        use ring::rand::SecureRandom;
        ring::rand::SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| Error::Protocol("ss: random failed"))?;

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

        // Build target data
        let target_data = build_target_data(&session.target, session.port, &[])?;

        let mut nonce = 0;
        let encrypted = encrypt_tcp_chunk(cipher, &key, &mut nonce, &target_data)?;

        let mut request = Vec::with_capacity(salt.len() + encrypted.len());
        request.extend_from_slice(&salt);
        request.extend_from_slice(&encrypted);
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("ss: failed to write request"))?;

        Ok(ShadowsocksOutboundSession {
            session_key: key,
            next_upload_nonce: nonce,
            cipher,
        })
    }
}
