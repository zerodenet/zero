// Shadowsocks outbound protocol — outbound.rs

use zero_core::{Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

/// Shadowsocks outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShadowsocksOutbound;

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
    ) -> Result<(), Error> {
        use super::shared::{aead_encrypt, build_target_data};

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

        // Encrypt: nonce is zero for first chunk
        let nonce = [0u8; 12];
        let encrypted = aead_encrypt(cipher, &key, &nonce, &target_data)?;

        // Write salt + encrypted chunk
        stream
            .write_all(&salt)
            .await
            .map_err(|_| Error::Io("ss: failed to write salt"))?;
        stream
            .write_all(&encrypted)
            .await
            .map_err(|_| Error::Io("ss: failed to write encrypted chunk"))?;

        Ok(())
    }
}
