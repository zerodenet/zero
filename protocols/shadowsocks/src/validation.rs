use alloc::format;
use alloc::string::{String, ToString};

use crate::CipherKind;

pub fn validate_cipher(cipher: &str) -> Result<CipherKind, String> {
    CipherKind::from_str(cipher).ok_or_else(|| format!("unknown cipher `{cipher}`"))
}

pub fn validate_password(cipher: &str, password: &str) -> Result<(), String> {
    let cipher = validate_cipher(cipher)?;
    if !cipher.is_blake3() {
        return Ok(());
    }

    #[cfg(feature = "blake3")]
    {
        let key = if matches!(
            cipher,
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm
        ) {
            password.rsplit(':').next().unwrap_or(password)
        } else {
            password
        };
        crate::decode_blake3_master_key(cipher, key.as_bytes())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(feature = "blake3"))]
    {
        let _ = password;
        Err("2022 cipher validation requires the `blake3` feature".into())
    }
}
