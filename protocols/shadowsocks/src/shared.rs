// Shadowsocks protocol constants and helpers — shared.rs
//
// SIP003 AEAD ciphers: aes-128-gcm, aes-256-gcm, chacha20-poly1305.

use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

pub const ADDR_TYPE_IPV4: u8 = 0x01;
pub const ADDR_TYPE_DOMAIN: u8 = 0x03;
pub const ADDR_TYPE_IPV6: u8 = 0x04;

/// AEAD cipher methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherKind {
    Aes128Gcm,
    Aes256Gcm,
    Chacha20Poly1305,
    Blake3Aes128Gcm,
    Blake3Aes256Gcm,
    Blake3Chacha20Poly1305,
}

impl CipherKind {
    pub fn key_len(&self) -> usize {
        match self {
            Self::Aes128Gcm | Self::Blake3Aes128Gcm => 16,
            Self::Aes256Gcm | Self::Blake3Aes256Gcm => 32,
            Self::Chacha20Poly1305 | Self::Blake3Chacha20Poly1305 => 32,
        }
    }

    pub fn salt_len(&self) -> usize {
        if self.is_blake3() {
            16
        } else {
            self.key_len()
        }
    }

    pub fn tag_len(&self) -> usize {
        16
    }

    pub fn is_blake3(&self) -> bool {
        matches!(self, Self::Blake3Aes128Gcm | Self::Blake3Aes256Gcm | Self::Blake3Chacha20Poly1305)
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "aes-128-gcm" => Some(Self::Aes128Gcm),
            "aes-256-gcm" => Some(Self::Aes256Gcm),
            "chacha20-ietf-poly1305" => Some(Self::Chacha20Poly1305),
            "2022-blake3-aes-128-gcm" => Some(Self::Blake3Aes128Gcm),
            "2022-blake3-aes-256-gcm" => Some(Self::Blake3Aes256Gcm),
            "2022-blake3-chacha20-poly1305" => Some(Self::Blake3Chacha20Poly1305),
            _ => None,
        }
    }

    #[cfg(feature = "crypto")]
    fn to_ring_algorithm(&self) -> &'static ring::aead::Algorithm {
        use ring::aead;
        match self {
            Self::Aes128Gcm | Self::Blake3Aes128Gcm => &aead::AES_128_GCM,
            Self::Aes256Gcm | Self::Blake3Aes256Gcm => &aead::AES_256_GCM,
            Self::Chacha20Poly1305 | Self::Blake3Chacha20Poly1305 => &aead::CHACHA20_POLY1305,
        }
    }
}

// — key derivation —

#[cfg(feature = "crypto")]
pub fn derive_key(password: &[u8], salt: &[u8], key_len: usize) -> Result<Vec<u8>, Error> {
    use ring::hkdf::Salt;
    let salt = Salt::new(ring::hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY, salt);
    let prk = salt.extract(password);
    let mut key = vec![0u8; key_len];
    prk.expand(&[b"ss-subkey"], ShadowsocksKeyLen(key_len))
        .and_then(|okm| okm.fill(&mut key))
        .map_err(|_| Error::Protocol("ss: key derivation failed"))?;
    Ok(key)
}

// — 2022 Blake3 KDF —

#[cfg(feature = "blake3")]
pub fn derive_key_blake3(password: &[u8], salt: &[u8], key_len: usize) -> Result<Vec<u8>, Error> {
    let mut key = vec![0u8; key_len];
    let mut hasher = blake3::Hasher::new_derive_key("shadowsocks 2022 session subkey");
    hasher.update(password);
    if !salt.is_empty() {
        hasher.update(salt);
    }
    let output = hasher.finalize();
    key.copy_from_slice(&output.as_bytes()[..key_len]);
    Ok(key)
}

#[cfg(feature = "crypto")]
struct ShadowsocksKeyLen(usize);

#[cfg(feature = "crypto")]
impl ring::hkdf::KeyType for ShadowsocksKeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

// — AEAD encrypt / decrypt —

#[cfg(feature = "crypto")]
pub fn aead_encrypt(
    cipher: CipherKind,
    key: &[u8],
    nonce: &[u8; 12],
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    let unbound = UnboundKey::new(cipher.to_ring_algorithm(), key)
        .map_err(|_| Error::Protocol("ss: invalid key"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut buf = Vec::with_capacity(2 + plaintext.len() + 16);
    buf.extend_from_slice(&(plaintext.len() as u16).to_be_bytes());
    buf.extend_from_slice(plaintext);
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("ss: encryption failed"))?;
    Ok(buf)
}

#[cfg(feature = "crypto")]
pub fn aead_decrypt(
    cipher: CipherKind,
    key: &[u8],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    if ciphertext.len() < cipher.tag_len() {
        return Err(Error::Protocol("ss: ciphertext too short"));
    }
    let unbound = UnboundKey::new(cipher.to_ring_algorithm(), key)
        .map_err(|_| Error::Protocol("ss: invalid key"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut buf = ciphertext.to_vec();
    let decrypted = key
        .open_in_place(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("ss: decryption failed"))?;
    // Strip 2-byte chunk length prefix added by aead_encrypt
    if decrypted.len() < 2 {
        return Err(Error::Protocol("ss: decrypted data too short"));
    }
    Ok(decrypted[2..].to_vec())
}

// — UDP AEAD (no chunk length prefix, per-packet salt + nonce) —

#[cfg(feature = "crypto")]
pub fn aead_encrypt_udp(
    cipher: CipherKind,
    key: &[u8],
    nonce: &[u8; 12],
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    let unbound = UnboundKey::new(cipher.to_ring_algorithm(), key)
        .map_err(|_| Error::Protocol("ss: invalid key"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut buf = Vec::with_capacity(plaintext.len() + cipher.tag_len());
    buf.extend_from_slice(plaintext);
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("ss: encryption failed"))?;
    Ok(buf)
}

#[cfg(feature = "crypto")]
pub fn aead_decrypt_udp(
    cipher: CipherKind,
    key: &[u8],
    nonce: &[u8; 12],
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    if ciphertext.len() < cipher.tag_len() {
        return Err(Error::Protocol("ss: ciphertext too short"));
    }
    let unbound = UnboundKey::new(cipher.to_ring_algorithm(), key)
        .map_err(|_| Error::Protocol("ss: invalid key"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(*nonce);
    let mut buf = ciphertext.to_vec();
    let decrypted = key
        .open_in_place(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("ss: decryption failed"))?;
    Ok(decrypted.to_vec())
}

// — address encode / decode —

pub fn encode_address(addr: &Address) -> Result<Vec<u8>, Error> {
    match addr {
        Address::Ipv4(bytes) => {
            let mut buf = Vec::with_capacity(5);
            buf.push(ADDR_TYPE_IPV4);
            buf.extend_from_slice(bytes);
            Ok(buf)
        }
        Address::Ipv6(bytes) => {
            let mut buf = Vec::with_capacity(17);
            buf.push(ADDR_TYPE_IPV6);
            buf.extend_from_slice(bytes);
            Ok(buf)
        }
        Address::Domain(domain) => {
            let b = domain.as_bytes();
            if b.is_empty() || b.len() > u8::MAX as usize {
                return Err(Error::Protocol("ss: invalid domain length"));
            }
            let mut buf = Vec::with_capacity(2 + b.len());
            buf.push(ADDR_TYPE_DOMAIN);
            buf.push(b.len() as u8);
            buf.extend_from_slice(b);
            Ok(buf)
        }
    }
}

pub fn decode_address(data: &[u8]) -> Result<(Address, usize), Error> {
    if data.is_empty() {
        return Err(Error::Protocol("ss: empty address data"));
    }
    match data[0] {
        ADDR_TYPE_IPV4 => {
            if data.len() < 5 {
                return Err(Error::Protocol("ss: truncated IPv4"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[1..5]);
            Ok((Address::Ipv4(bytes), 5))
        }
        ADDR_TYPE_IPV6 => {
            if data.len() < 17 {
                return Err(Error::Protocol("ss: truncated IPv6"));
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[1..17]);
            Ok((Address::Ipv6(bytes), 17))
        }
        ADDR_TYPE_DOMAIN => {
            let len = data[1] as usize;
            if data.len() < 2 + len {
                return Err(Error::Protocol("ss: truncated domain"));
            }
            let domain = String::from_utf8(data[2..2 + len].to_vec())
                .map_err(|_| Error::Protocol("ss: invalid domain UTF-8"))?;
            Ok((Address::Domain(domain), 2 + len))
        }
        _ => Err(Error::Unsupported("ss: unknown address type")),
    }
}

/// Build target + payload bytes. Format: [addr][port:2][payload]
pub fn build_target_data(addr: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let addr_bytes = encode_address(addr)?;
    let mut buf = Vec::with_capacity(addr_bytes.len() + 2 + payload.len());
    buf.extend_from_slice(&addr_bytes);
    buf.extend_from_slice(&port.to_be_bytes());
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Parse target + payload bytes. Returns (address, port, remaining payload offset).
pub fn parse_target_data(data: &[u8]) -> Result<(Address, u16, usize), Error> {
    let (addr, addr_end) = decode_address(data)?;
    if data.len() < addr_end + 2 {
        return Err(Error::Protocol("ss: truncated port"));
    }
    let port = u16::from_be_bytes([data[addr_end], data[addr_end + 1]]);
    Ok((addr, port, addr_end + 2))
}

/// Read exact number of bytes from stream.
pub async fn read_exact<S: AsyncSocket>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("ss: read failed"))?;
        if n == 0 {
            return Err(Error::Io("ss: unexpected EOF"));
        }
        offset += n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cipher_kind_from_str() {
        assert_eq!(CipherKind::from_str("aes-128-gcm"), Some(CipherKind::Aes128Gcm));
        assert_eq!(CipherKind::from_str("aes-256-gcm"), Some(CipherKind::Aes256Gcm));
        assert_eq!(
            CipherKind::from_str("chacha20-ietf-poly1305"),
            Some(CipherKind::Chacha20Poly1305)
        );
        assert_eq!(CipherKind::from_str("nonexistent"), None);
    }

    #[test]
    fn test_cipher_key_len() {
        assert_eq!(CipherKind::Aes128Gcm.key_len(), 16);
        assert_eq!(CipherKind::Aes256Gcm.key_len(), 32);
        assert_eq!(CipherKind::Chacha20Poly1305.key_len(), 32);
    }

    #[test]
    fn test_address_roundtrip() {
        let cases = vec![
            Address::Ipv4([127, 0, 0, 1]),
            Address::Domain("example.com".into()),
            Address::Ipv6([0; 16]),
        ];
        for addr in cases {
            let encoded = encode_address(&addr).unwrap();
            let (decoded, consumed) = decode_address(&encoded).unwrap();
            assert_eq!(addr, decoded);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn test_target_data_roundtrip() {
        let addr = Address::Domain("example.com".into());
        let data = build_target_data(&addr, 443, b"hello").unwrap();
        let (parsed_addr, port, offset) = parse_target_data(&data).unwrap();
        assert_eq!(parsed_addr, addr);
        assert_eq!(port, 443);
        assert_eq!(&data[offset..], b"hello");
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_aead_roundtrip() {
        let cipher = CipherKind::Aes128Gcm;
        let password = b"test-password";
        let salt = [0x42u8; 16];
        let key = derive_key(password, &salt, cipher.key_len()).unwrap();
        let nonce = [0x00u8; 12];
        let plaintext = b"hello shadowsocks";
        let ct = aead_encrypt(cipher, &key, &nonce, plaintext).unwrap();
        let pt = aead_decrypt(cipher, &key, &nonce, &ct).unwrap();
        assert_eq!(pt, plaintext);
    }
}
