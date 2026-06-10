// Shadowsocks protocol constants and helpers.
//
// SIP003 AEAD ciphers: aes-128-gcm, aes-256-gcm, chacha20-ietf-poly1305.

use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

pub const ADDR_TYPE_IPV4: u8 = 0x01;
pub const ADDR_TYPE_DOMAIN: u8 = 0x03;
pub const ADDR_TYPE_IPV6: u8 = 0x04;
pub const TCP_CHUNK_SIZE_LEN: usize = 2;
pub const MAX_TCP_PAYLOAD_SIZE: usize = 0x3fff;

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
            self.key_len()
        } else {
            self.key_len()
        }
    }

    pub fn udp_salt_len(&self) -> usize {
        match self {
            Self::Blake3Aes128Gcm | Self::Blake3Aes256Gcm => 12,
            Self::Blake3Chacha20Poly1305 => 24,
            _ => self.salt_len(),
        }
    }

    pub fn tag_len(&self) -> usize {
        16
    }

    pub fn is_blake3(&self) -> bool {
        matches!(
            self,
            Self::Blake3Aes128Gcm | Self::Blake3Aes256Gcm | Self::Blake3Chacha20Poly1305
        )
    }

    #[allow(clippy::should_implement_trait)]
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

// Key derivation.

#[cfg(feature = "crypto")]
pub fn derive_key(password: &[u8], salt: &[u8], key_len: usize) -> Result<Vec<u8>, Error> {
    use ring::hkdf::Salt;
    let master_key = evp_bytes_to_key(password, key_len);
    let salt = Salt::new(ring::hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY, salt);
    let prk = salt.extract(&master_key);
    let mut key = vec![0u8; key_len];
    prk.expand(&[b"ss-subkey"], ShadowsocksKeyLen(key_len))
        .and_then(|okm| okm.fill(&mut key))
        .map_err(|_| Error::Protocol("ss: key derivation failed"))?;
    Ok(key)
}

#[cfg(feature = "crypto")]
fn evp_bytes_to_key(password: &[u8], key_len: usize) -> Vec<u8> {
    use md5::{Digest, Md5};

    let mut key = Vec::with_capacity(key_len);
    let mut previous = Vec::new();
    while key.len() < key_len {
        let mut hasher = Md5::new();
        if !previous.is_empty() {
            hasher.update(&previous);
        }
        hasher.update(password);
        previous = hasher.finalize().to_vec();
        key.extend_from_slice(&previous);
    }
    key.truncate(key_len);
    key
}

// 2022 Blake3 KDF.

#[cfg(feature = "blake3")]
pub fn derive_key_blake3(
    master_key: &[u8],
    material: &[u8],
    key_len: usize,
) -> Result<Vec<u8>, Error> {
    let mut key = vec![0u8; key_len];
    let mut hasher = blake3::Hasher::new_derive_key("shadowsocks 2022 session subkey");
    hasher.update(master_key);
    if !material.is_empty() {
        hasher.update(material);
    }
    hasher.finalize_xof().fill(&mut key);
    Ok(key)
}

#[cfg(feature = "blake3")]
pub fn decode_blake3_master_key(cipher: CipherKind, password: &[u8]) -> Result<Vec<u8>, Error> {
    use base64::{
        alphabet,
        engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
        Engine,
    };

    let password = core::str::from_utf8(password)
        .map_err(|_| Error::Protocol("ss: 2022 password must be utf-8 base64"))?;
    let password = match cipher {
        CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
            password.rsplit(':').next().unwrap_or(password)
        }
        CipherKind::Blake3Chacha20Poly1305 => password,
        _ => return Err(Error::Protocol("ss: cipher is not a 2022 method")),
    };

    const ENGINE: GeneralPurpose = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_encode_padding(true)
            .with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );

    let key = ENGINE
        .decode(password)
        .map_err(|_| Error::Protocol("ss: invalid 2022 base64 password"))?;
    if key.len() != cipher.key_len() {
        return Err(Error::Protocol("ss: invalid 2022 password key length"));
    }
    Ok(key)
}

#[cfg(feature = "crypto")]
pub fn derive_session_key(
    cipher: CipherKind,
    password: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, Error> {
    if cipher.is_blake3() {
        #[cfg(feature = "blake3")]
        {
            let master_key = decode_blake3_master_key(cipher, password)?;
            return derive_key_blake3(&master_key, salt, cipher.key_len());
        }
        #[cfg(not(feature = "blake3"))]
        return Err(Error::Protocol(
            "ss: blake3 key derivation requires `blake3` feature",
        ));
    }
    derive_key(password, salt, cipher.key_len())
}

#[cfg(feature = "crypto")]
pub fn derive_udp_packet_key(
    cipher: CipherKind,
    password: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, Error> {
    if !cipher.is_blake3() {
        return derive_key(password, salt, cipher.key_len());
    }

    #[cfg(feature = "blake3")]
    {
        let master_key = decode_blake3_master_key(cipher, password)?;
        match cipher {
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
                if salt.len() != 12 {
                    return Err(Error::Protocol("ss: invalid 2022 aes udp nonce length"));
                }
                let mut session_id = [0u8; 8];
                session_id.copy_from_slice(&salt[..8]);
                derive_key_blake3(&master_key, &session_id, cipher.key_len())
            }
            CipherKind::Blake3Chacha20Poly1305 => {
                if salt.len() != 24 {
                    return Err(Error::Protocol("ss: invalid 2022 chacha udp nonce length"));
                }
                Ok(master_key)
            }
            _ => Err(Error::Protocol("ss: cipher is not a 2022 method")),
        }
    }
    #[cfg(not(feature = "blake3"))]
    {
        let _ = (cipher, password, salt);
        Err(Error::Protocol(
            "ss: blake3 key derivation requires `blake3` feature",
        ))
    }
}

#[cfg(feature = "crypto")]
pub fn encode_udp_datagram_2022(
    cipher: CipherKind,
    password: &[u8],
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let master_key = {
        #[cfg(feature = "blake3")]
        {
            decode_blake3_master_key(cipher, password)?
        }
        #[cfg(not(feature = "blake3"))]
        {
            let _ = (cipher, password);
            return Err(Error::Protocol(
                "ss: blake3 key derivation requires `blake3` feature",
            ));
        }
    };

    let target_data = build_target_data(target, port, payload)?;
    let mut packet = Vec::with_capacity(64 + target_data.len() + cipher.tag_len());
    let session_id = random_u64()?;
    let packet_id = random_u64()?;

    if cipher == CipherKind::Blake3Chacha20Poly1305 {
        let mut nonce = [0u8; 24];
        fill_random(&mut nonce)?;
        packet.extend_from_slice(&nonce);
    }

    packet.extend_from_slice(&session_id.to_be_bytes());
    packet.extend_from_slice(&packet_id.to_be_bytes());
    packet.push(0);
    packet.extend_from_slice(&now_unix_seconds().to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&target_data);

    match cipher {
        CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
            let mut header = [0u8; 16];
            header.copy_from_slice(&packet[..16]);
            let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
            let encrypted = aead_encrypt_udp(cipher, &session_key, &header[4..16], &packet[16..])?;
            encrypt_aes_2022_header(cipher, &master_key, &mut header)?;

            let mut out = Vec::with_capacity(header.len() + encrypted.len());
            out.extend_from_slice(&header);
            out.extend_from_slice(&encrypted);
            Ok(out)
        }
        CipherKind::Blake3Chacha20Poly1305 => {
            let encrypted = aead_encrypt_udp(cipher, &master_key, &packet[..24], &packet[24..])?;
            let mut out = Vec::with_capacity(24 + encrypted.len());
            out.extend_from_slice(&packet[..24]);
            out.extend_from_slice(&encrypted);
            Ok(out)
        }
        _ => Err(Error::Protocol("ss: cipher is not a 2022 method")),
    }
}

#[cfg(feature = "crypto")]
pub fn decode_udp_datagram_2022(
    cipher: CipherKind,
    password: &[u8],
    datagram: &[u8],
) -> Result<(Address, u16, Vec<u8>), Error> {
    let master_key = {
        #[cfg(feature = "blake3")]
        {
            decode_blake3_master_key(cipher, password)?
        }
        #[cfg(not(feature = "blake3"))]
        {
            let _ = (cipher, password, datagram);
            return Err(Error::Protocol(
                "ss: blake3 key derivation requires `blake3` feature",
            ));
        }
    };

    let plain = match cipher {
        CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
            if datagram.len() < 16 + cipher.tag_len() {
                return Err(Error::Protocol("ss: udp datagram too short"));
            }
            let mut header = [0u8; 16];
            header.copy_from_slice(&datagram[..16]);
            decrypt_aes_2022_header(cipher, &master_key, &mut header)?;
            let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
            let message = aead_decrypt_udp(cipher, &session_key, &header[4..16], &datagram[16..])?;
            let mut plain = Vec::with_capacity(header.len() + message.len());
            plain.extend_from_slice(&header);
            plain.extend_from_slice(&message);
            plain
        }
        CipherKind::Blake3Chacha20Poly1305 => {
            if datagram.len() < 24 + cipher.tag_len() {
                return Err(Error::Protocol("ss: udp datagram too short"));
            }
            aead_decrypt_udp(cipher, &master_key, &datagram[..24], &datagram[24..])?
        }
        _ => return Err(Error::Protocol("ss: cipher is not a 2022 method")),
    };

    parse_udp_2022_plain(&plain)
}

#[cfg(feature = "crypto")]
fn parse_udp_2022_plain(plain: &[u8]) -> Result<(Address, u16, Vec<u8>), Error> {
    if plain.len() < 8 + 8 + 1 + 8 + 2 {
        return Err(Error::Protocol("ss: udp 2022 packet too short"));
    }

    let socket_type = plain[16];
    let mut cursor = match socket_type {
        0 => 17 + 8,
        1 => 17 + 8 + 8,
        _ => return Err(Error::Protocol("ss: invalid udp 2022 socket type")),
    };

    if plain.len() < cursor + 2 {
        return Err(Error::Protocol("ss: udp 2022 packet too short"));
    }
    let padding_len = u16::from_be_bytes([plain[cursor], plain[cursor + 1]]) as usize;
    cursor += 2;
    if plain.len() < cursor + padding_len {
        return Err(Error::Protocol("ss: invalid udp 2022 padding length"));
    }
    cursor += padding_len;

    let (target, port, payload_offset) = parse_target_data(&plain[cursor..])?;
    Ok((target, port, plain[cursor + payload_offset..].to_vec()))
}

#[cfg(feature = "crypto")]
fn encrypt_aes_2022_header(
    cipher: CipherKind,
    master_key: &[u8],
    header: &mut [u8; 16],
) -> Result<(), Error> {
    use aes::{
        cipher::{BlockEncrypt, KeyInit},
        Aes128, Aes256,
    };

    match cipher {
        CipherKind::Blake3Aes128Gcm => {
            let cipher = Aes128::new_from_slice(master_key)
                .map_err(|_| Error::Protocol("ss: invalid key"))?;
            cipher.encrypt_block(header.into());
            Ok(())
        }
        CipherKind::Blake3Aes256Gcm => {
            let cipher = Aes256::new_from_slice(master_key)
                .map_err(|_| Error::Protocol("ss: invalid key"))?;
            cipher.encrypt_block(header.into());
            Ok(())
        }
        _ => Err(Error::Protocol("ss: cipher is not a 2022 aes method")),
    }
}

#[cfg(feature = "crypto")]
fn decrypt_aes_2022_header(
    cipher: CipherKind,
    master_key: &[u8],
    header: &mut [u8; 16],
) -> Result<(), Error> {
    use aes::{
        cipher::{BlockDecrypt, KeyInit},
        Aes128, Aes256,
    };

    match cipher {
        CipherKind::Blake3Aes128Gcm => {
            let cipher = Aes128::new_from_slice(master_key)
                .map_err(|_| Error::Protocol("ss: invalid key"))?;
            cipher.decrypt_block(header.into());
            Ok(())
        }
        CipherKind::Blake3Aes256Gcm => {
            let cipher = Aes256::new_from_slice(master_key)
                .map_err(|_| Error::Protocol("ss: invalid key"))?;
            cipher.decrypt_block(header.into());
            Ok(())
        }
        _ => Err(Error::Protocol("ss: cipher is not a 2022 aes method")),
    }
}

#[cfg(feature = "crypto")]
fn random_u64() -> Result<u64, Error> {
    let mut bytes = [0u8; 8];
    fill_random(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

#[cfg(feature = "crypto")]
fn fill_random(bytes: &mut [u8]) -> Result<(), Error> {
    use ring::rand::SecureRandom;
    ring::rand::SystemRandom::new()
        .fill(bytes)
        .map_err(|_| Error::Protocol("ss: random failed"))
}

#[cfg(feature = "crypto")]
fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(feature = "crypto")]
struct ShadowsocksKeyLen(usize);

#[cfg(feature = "crypto")]
impl ring::hkdf::KeyType for ShadowsocksKeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

// AEAD encrypt / decrypt.

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
    let mut buf = Vec::with_capacity(plaintext.len() + cipher.tag_len());
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
    Ok(decrypted.to_vec())
}

#[cfg(feature = "crypto")]
pub fn tcp_nonce(counter: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(&counter.to_le_bytes());
    nonce
}

#[cfg(feature = "crypto")]
pub fn encrypt_tcp_chunk(
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    if payload.len() > MAX_TCP_PAYLOAD_SIZE {
        return Err(Error::Protocol("ss: tcp chunk too large"));
    }

    let length_nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    let encrypted_length = aead_encrypt(
        cipher,
        key,
        &length_nonce,
        &(payload.len() as u16).to_be_bytes(),
    )?;

    let payload_nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    let encrypted_payload = aead_encrypt(cipher, key, &payload_nonce, payload)?;

    let mut chunk = Vec::with_capacity(encrypted_length.len() + encrypted_payload.len());
    chunk.extend_from_slice(&encrypted_length);
    chunk.extend_from_slice(&encrypted_payload);
    Ok(chunk)
}

#[cfg(feature = "crypto")]
pub fn decrypt_tcp_chunk_length(
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    encrypted_length: &[u8],
) -> Result<usize, Error> {
    if encrypted_length.len() != TCP_CHUNK_SIZE_LEN + cipher.tag_len() {
        return Err(Error::Protocol("ss: invalid encrypted length size"));
    }

    let nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    let plain = aead_decrypt(cipher, key, &nonce, encrypted_length)?;
    if plain.len() != TCP_CHUNK_SIZE_LEN {
        return Err(Error::Protocol("ss: invalid decrypted length size"));
    }

    let payload_len = u16::from_be_bytes([plain[0], plain[1]]) as usize;
    if payload_len > MAX_TCP_PAYLOAD_SIZE {
        return Err(Error::Protocol("ss: tcp chunk too large"));
    }
    Ok(payload_len)
}

#[cfg(feature = "crypto")]
pub fn decrypt_tcp_chunk_payload(
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    expected_len: usize,
    encrypted_payload: &[u8],
) -> Result<Vec<u8>, Error> {
    if encrypted_payload.len() != expected_len + cipher.tag_len() {
        return Err(Error::Protocol("ss: invalid encrypted payload size"));
    }

    let nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    let plain = aead_decrypt(cipher, key, &nonce, encrypted_payload)?;
    if plain.len() != expected_len {
        return Err(Error::Protocol("ss: invalid decrypted payload size"));
    }
    Ok(plain)
}

#[cfg(feature = "crypto")]
pub async fn read_tcp_chunk<S: AsyncSocket>(
    stream: &mut S,
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
) -> Result<Vec<u8>, Error> {
    let mut encrypted_length = vec![0u8; TCP_CHUNK_SIZE_LEN + cipher.tag_len()];
    read_exact(stream, &mut encrypted_length).await?;
    let payload_len = decrypt_tcp_chunk_length(cipher, key, nonce_counter, &encrypted_length)?;

    let mut encrypted_payload = vec![0u8; payload_len + cipher.tag_len()];
    read_exact(stream, &mut encrypted_payload).await?;
    decrypt_tcp_chunk_payload(cipher, key, nonce_counter, payload_len, &encrypted_payload)
}

// UDP AEAD uses per-packet salt and a fixed zero nonce.

#[cfg(feature = "crypto")]
pub fn aead_encrypt_udp(
    cipher: CipherKind,
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    if cipher == CipherKind::Blake3Chacha20Poly1305 {
        use chacha20poly1305::{
            aead::{AeadInPlace, KeyInit},
            XChaCha20Poly1305, XNonce,
        };
        if nonce.len() != 24 {
            return Err(Error::Protocol("ss: invalid xchacha nonce length"));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(key)
            .map_err(|_| Error::Protocol("ss: invalid key"))?;
        let mut buf =
            Vec::with_capacity(plaintext.len() + CipherKind::Blake3Chacha20Poly1305.tag_len());
        buf.extend_from_slice(plaintext);
        cipher
            .encrypt_in_place(XNonce::from_slice(nonce), b"", &mut buf)
            .map_err(|_| Error::Protocol("ss: encryption failed"))?;
        return Ok(buf);
    }

    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    let nonce: &[u8; 12] = nonce
        .try_into()
        .map_err(|_| Error::Protocol("ss: invalid nonce length"))?;
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
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    if cipher == CipherKind::Blake3Chacha20Poly1305 {
        use chacha20poly1305::{
            aead::{AeadInPlace, KeyInit},
            XChaCha20Poly1305, XNonce,
        };
        if nonce.len() != 24 {
            return Err(Error::Protocol("ss: invalid xchacha nonce length"));
        }
        if ciphertext.len() < cipher.tag_len() {
            return Err(Error::Protocol("ss: ciphertext too short"));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(key)
            .map_err(|_| Error::Protocol("ss: invalid key"))?;
        let mut buf = ciphertext.to_vec();
        cipher
            .decrypt_in_place(XNonce::from_slice(nonce), b"", &mut buf)
            .map_err(|_| Error::Protocol("ss: decryption failed"))?;
        return Ok(buf);
    }

    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey};
    if ciphertext.len() < cipher.tag_len() {
        return Err(Error::Protocol("ss: ciphertext too short"));
    }
    let nonce: &[u8; 12] = nonce
        .try_into()
        .map_err(|_| Error::Protocol("ss: invalid nonce length"))?;
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

// Address encode / decode.

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

// TCP stream helpers.

/// Derive the download key from password and salt.
///
/// Handles both standard (HKDF) and blake3 key derivation based on cipher type.
/// For outbound connections, the download key is derived from the server's
/// response salt. For inbound connections, from the client's request salt.
#[cfg(feature = "crypto")]
pub fn derive_download_key(
    cipher: CipherKind,
    password: &[u8],
    salt: &[u8],
) -> Result<Vec<u8>, Error> {
    derive_session_key(cipher, password, salt)
}

/// Encrypt a TCP chunk and write it to the stream.
///
/// Wraps [`encrypt_tcp_chunk`] + [`AsyncSocket::write_all`]. Each call
/// consumes `payload.len()` plain bytes (up to [`MAX_TCP_PAYLOAD_SIZE`])
/// and writes the encrypted AEAD chunk (encrypted length + encrypted payload)
/// to the stream.
#[cfg(feature = "crypto")]
pub async fn write_tcp_chunk<S: AsyncSocket>(
    stream: &mut S,
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    payload: &[u8],
) -> Result<(), Error> {
    let chunk = encrypt_tcp_chunk(cipher, key, nonce_counter, payload)?;
    stream
        .write_all(&chunk)
        .await
        .map_err(|_| Error::Io("ss: write failed"))
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn test_cipher_kind_from_str() {
        assert_eq!(
            CipherKind::from_str("aes-128-gcm"),
            Some(CipherKind::Aes128Gcm)
        );
        assert_eq!(
            CipherKind::from_str("aes-256-gcm"),
            Some(CipherKind::Aes256Gcm)
        );
        assert_eq!(
            CipherKind::from_str("chacha20-ietf-poly1305"),
            Some(CipherKind::Chacha20Poly1305)
        );
        assert_eq!(
            CipherKind::from_str("2022-blake3-aes-128-gcm"),
            Some(CipherKind::Blake3Aes128Gcm)
        );
        assert_eq!(
            CipherKind::from_str("2022-blake3-aes-256-gcm"),
            Some(CipherKind::Blake3Aes256Gcm)
        );
        assert_eq!(
            CipherKind::from_str("2022-blake3-chacha20-poly1305"),
            Some(CipherKind::Blake3Chacha20Poly1305)
        );
        assert_eq!(CipherKind::from_str("nonexistent"), None);
    }

    #[test]
    fn test_cipher_key_len() {
        assert_eq!(CipherKind::Aes128Gcm.key_len(), 16);
        assert_eq!(CipherKind::Aes256Gcm.key_len(), 32);
        assert_eq!(CipherKind::Chacha20Poly1305.key_len(), 32);
        assert_eq!(CipherKind::Blake3Aes128Gcm.key_len(), 16);
        assert_eq!(CipherKind::Blake3Aes256Gcm.key_len(), 32);
        assert_eq!(CipherKind::Blake3Chacha20Poly1305.key_len(), 32);
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

    #[cfg(feature = "crypto")]
    #[test]
    fn test_tcp_chunk_roundtrip() {
        let cipher = CipherKind::Aes128Gcm;
        let password = b"test-password";
        let salt = [0x42u8; 16];
        let key = derive_key(password, &salt, cipher.key_len()).unwrap();
        let plaintext = b"hello shadowsocks";
        let mut encrypt_nonce = 0;
        let chunk = encrypt_tcp_chunk(cipher, &key, &mut encrypt_nonce, plaintext).unwrap();
        assert_eq!(encrypt_nonce, 2);

        let mut decrypt_nonce = 0;
        let length_size = TCP_CHUNK_SIZE_LEN + cipher.tag_len();
        let payload_len =
            decrypt_tcp_chunk_length(cipher, &key, &mut decrypt_nonce, &chunk[..length_size])
                .unwrap();
        let pt = decrypt_tcp_chunk_payload(
            cipher,
            &key,
            &mut decrypt_nonce,
            payload_len,
            &chunk[length_size..],
        )
        .unwrap();
        assert_eq!(decrypt_nonce, 2);
        assert_eq!(pt, plaintext);
    }
}
