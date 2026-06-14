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

    pub const fn is_blake3(&self) -> bool {
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
    #[cfg(feature = "blake3")]
    {
        let master_key = decode_blake3_master_key(cipher, password)?;

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

        return match cipher {
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
                let mut header = [0u8; 16];
                header.copy_from_slice(&packet[..16]);
                let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
                let encrypted =
                    aead_encrypt_udp(cipher, &session_key, &header[4..16], &packet[16..])?;
                encrypt_aes_2022_header(cipher, &master_key, &mut header)?;

                let mut out = Vec::with_capacity(header.len() + encrypted.len());
                out.extend_from_slice(&header);
                out.extend_from_slice(&encrypted);
                Ok(out)
            }
            CipherKind::Blake3Chacha20Poly1305 => {
                let encrypted =
                    aead_encrypt_udp(cipher, &master_key, &packet[..24], &packet[24..])?;
                let mut out = Vec::with_capacity(24 + encrypted.len());
                out.extend_from_slice(&packet[..24]);
                out.extend_from_slice(&encrypted);
                Ok(out)
            }
            _ => Err(Error::Protocol("ss: cipher is not a 2022 method")),
        };
    }

    #[cfg(not(feature = "blake3"))]
    {
        let _ = (cipher, password, target, port, payload);
        Err(Error::Protocol(
            "ss: 2022 udp datagram requires `blake3` feature",
        ))
    }
}

/// Encode a Shadowsocks 2022 UDP **server-to-client** response datagram
/// (SIP022 3.2.3, socket type 1). The server generates a fresh server session
/// id / packet id for the separate header (or merged into the body for the
/// ChaCha20 variant) and echoes `client_session_id` in the body so the client
/// can map the response to its session.
#[cfg(feature = "crypto")]
pub fn encode_udp_response_2022(
    cipher: CipherKind,
    password: &[u8],
    client_session_id: u64,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    #[cfg(feature = "blake3")]
    {
        let master_key = decode_blake3_master_key(cipher, password)?;

        let target_data = build_target_data(target, port, payload)?;
        let mut packet = Vec::with_capacity(64 + target_data.len() + cipher.tag_len());
        let server_session_id = random_u64()?;
        let server_packet_id = random_u64()?;

        if cipher == CipherKind::Blake3Chacha20Poly1305 {
            let mut nonce = [0u8; 24];
            fill_random(&mut nonce)?;
            packet.extend_from_slice(&nonce);
        }

        packet.extend_from_slice(&server_session_id.to_be_bytes());
        packet.extend_from_slice(&server_packet_id.to_be_bytes());
        packet.push(SS_2022_HEADER_TYPE_SERVER_PACKET);
        packet.extend_from_slice(&now_unix_seconds().to_be_bytes());
        packet.extend_from_slice(&client_session_id.to_be_bytes());
        packet.extend_from_slice(&0u16.to_be_bytes());
        packet.extend_from_slice(&target_data);

        return match cipher {
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
                let mut header = [0u8; 16];
                header.copy_from_slice(&packet[..16]);
                let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
                let encrypted =
                    aead_encrypt_udp(cipher, &session_key, &header[4..16], &packet[16..])?;
                encrypt_aes_2022_header(cipher, &master_key, &mut header)?;

                let mut out = Vec::with_capacity(header.len() + encrypted.len());
                out.extend_from_slice(&header);
                out.extend_from_slice(&encrypted);
                Ok(out)
            }
            CipherKind::Blake3Chacha20Poly1305 => {
                let encrypted =
                    aead_encrypt_udp(cipher, &master_key, &packet[..24], &packet[24..])?;
                let mut out = Vec::with_capacity(24 + encrypted.len());
                out.extend_from_slice(&packet[..24]);
                out.extend_from_slice(&encrypted);
                Ok(out)
            }
            _ => Err(Error::Protocol("ss: cipher is not a 2022 method")),
        };
    }

    #[cfg(not(feature = "blake3"))]
    {
        let _ = (cipher, password, client_session_id, target, port, payload);
        Err(Error::Protocol(
            "ss: 2022 udp response requires `blake3` feature",
        ))
    }
}

#[cfg(feature = "crypto")]
pub fn decode_udp_datagram_2022(
    cipher: CipherKind,
    password: &[u8],
    datagram: &[u8],
) -> Result<(Address, u16, Vec<u8>), Error> {
    #[cfg(feature = "blake3")]
    {
        let master_key = decode_blake3_master_key(cipher, password)?;

        let plain = match cipher {
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
                if datagram.len() < 16 + cipher.tag_len() {
                    return Err(Error::Protocol("ss: udp datagram too short"));
                }
                let mut header = [0u8; 16];
                header.copy_from_slice(&datagram[..16]);
                decrypt_aes_2022_header(cipher, &master_key, &mut header)?;
                let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
                let message =
                    aead_decrypt_udp(cipher, &session_key, &header[4..16], &datagram[16..])?;
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

        return match parse_udp_2022_plain(&plain) {
            Ok((target, port, payload, _session_id)) => Ok((target, port, payload)),
            Err(e) => Err(e),
        };
    }

    #[cfg(not(feature = "blake3"))]
    {
        let _ = (cipher, password, datagram);
        Err(Error::Protocol(
            "ss: 2022 udp datagram requires `blake3` feature",
        ))
    }
}

/// Decode a 2022 UDP datagram, also returning the separate-header session id.
///
/// A server uses this to recover the client session id from an incoming
/// client packet (type 0) so it can echo it in server-to-client responses.
#[cfg(feature = "crypto")]
pub fn decode_udp_datagram_2022_session(
    cipher: CipherKind,
    password: &[u8],
    datagram: &[u8],
) -> Result<(Address, u16, Vec<u8>, u64), Error> {
    #[cfg(feature = "blake3")]
    {
        let master_key = decode_blake3_master_key(cipher, password)?;

        let plain = match cipher {
            CipherKind::Blake3Aes128Gcm | CipherKind::Blake3Aes256Gcm => {
                if datagram.len() < 16 + cipher.tag_len() {
                    return Err(Error::Protocol("ss: udp datagram too short"));
                }
                let mut header = [0u8; 16];
                header.copy_from_slice(&datagram[..16]);
                decrypt_aes_2022_header(cipher, &master_key, &mut header)?;
                let session_key = derive_key_blake3(&master_key, &header[..8], cipher.key_len())?;
                let message =
                    aead_decrypt_udp(cipher, &session_key, &header[4..16], &datagram[16..])?;
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

        return parse_udp_2022_plain(&plain);
    }

    #[cfg(not(feature = "blake3"))]
    {
        let _ = (cipher, password, datagram);
        Err(Error::Protocol(
            "ss: 2022 udp datagram requires `blake3` feature",
        ))
    }
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
fn parse_udp_2022_plain(plain: &[u8]) -> Result<(Address, u16, Vec<u8>, u64), Error> {
    if plain.len() < 8 + 8 + 1 + 8 + 2 {
        return Err(Error::Protocol("ss: udp 2022 packet too short"));
    }

    // The separate-header session id occupies the first 8 bytes (for AES it is
    // the separate-header session id; for ChaCha20 it is the body session id).
    let session_id = u64::from_be_bytes([
        plain[0], plain[1], plain[2], plain[3], plain[4], plain[5], plain[6], plain[7],
    ]);

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
    Ok((
        target,
        port,
        plain[cursor + payload_offset..].to_vec(),
        session_id,
    ))
}

// 2022 edition TCP header (SIP022 section 3.1.3).
//
// Request stream:  salt | fixed-header-chunk(nonce 0) | var-header-chunk(nonce 1)
//                          | [ length-chunk | payload-chunk ]*
// Response stream: salt | fixed-header-chunk(nonce 0, acts as first length chunk)
//                          | payload-chunk(nonce 1) | [ length-chunk | payload-chunk ]*
//
// The fixed/var headers and the response header are each a single AEAD
// operation (one nonce increment), unlike the legacy length+payload pair
// (two operations) used for body chunks.

pub const SS_2022_HEADER_TYPE_CLIENT_STREAM: u8 = 0;
pub const SS_2022_HEADER_TYPE_SERVER_STREAM: u8 = 1;
/// SIP022 3.2.3 UDP main-header socket types (same numeric values as streams).
pub const SS_2022_HEADER_TYPE_CLIENT_PACKET: u8 = 0;
pub const SS_2022_HEADER_TYPE_SERVER_PACKET: u8 = 1;
pub const SS_2022_MAX_PADDING_LENGTH: usize = 900;
/// Messages with over this many seconds of time skew are treated as replay.
pub const SS_2022_TIMESTAMP_WINDOW_SECS: u64 = 30;
/// Fixed request header is always 11 bytes: type(1) + timestamp(8) + length(2).
pub const SS_2022_REQUEST_FIXED_HEADER_LEN: usize = 11;

/// Build the request fixed-length header plaintext (11 bytes).
pub fn build_2022_request_fixed_header(
    timestamp: u64,
    var_header_len: u16,
) -> [u8; SS_2022_REQUEST_FIXED_HEADER_LEN] {
    let mut buf = [0u8; SS_2022_REQUEST_FIXED_HEADER_LEN];
    buf[0] = SS_2022_HEADER_TYPE_CLIENT_STREAM;
    buf[1..9].copy_from_slice(&timestamp.to_be_bytes());
    buf[9..11].copy_from_slice(&var_header_len.to_be_bytes());
    buf
}

/// Parse the request fixed-length header plaintext.
/// Returns `(type, timestamp, var_header_len)`.
pub fn parse_2022_request_fixed_header(plain: &[u8]) -> Result<(u8, u64, u16), Error> {
    if plain.len() != SS_2022_REQUEST_FIXED_HEADER_LEN {
        return Err(Error::Protocol("ss: 2022 request fixed header bad length"));
    }
    let mut ts = [0u8; 8];
    ts.copy_from_slice(&plain[1..9]);
    let timestamp = u64::from_be_bytes(ts);
    let var_len = u16::from_be_bytes([plain[9], plain[10]]);
    Ok((plain[0], timestamp, var_len))
}

/// Build the request variable-length header plaintext.
///
/// Layout: `ATYP + addr + port(2 BE) + padding_length(2 BE) + padding + initial_payload`.
pub fn build_2022_request_var_header(
    addr: &Address,
    port: u16,
    padding: &[u8],
    initial_payload: &[u8],
) -> Result<Vec<u8>, Error> {
    if padding.len() > SS_2022_MAX_PADDING_LENGTH {
        return Err(Error::Protocol("ss: 2022 padding exceeds max"));
    }
    let addr_bytes = encode_address(addr)?;
    let mut buf =
        Vec::with_capacity(addr_bytes.len() + 2 + 2 + padding.len() + initial_payload.len());
    buf.extend_from_slice(&addr_bytes);
    buf.extend_from_slice(&port.to_be_bytes());
    buf.extend_from_slice(&(padding.len() as u16).to_be_bytes());
    buf.extend_from_slice(padding);
    buf.extend_from_slice(initial_payload);
    Ok(buf)
}

/// Parse the request variable-length header plaintext.
///
/// Returns `(target, port, initial_payload)`. Padding is validated and discarded.
/// Rejects headers with neither initial payload nor padding per SIP022 3.1.3.
pub fn parse_2022_request_var_header(plain: &[u8]) -> Result<(Address, u16, Vec<u8>), Error> {
    let (addr, addr_end) = decode_address(plain)?;
    if plain.len() < addr_end + 2 {
        return Err(Error::Protocol("ss: 2022 var header truncated port"));
    }
    let port = u16::from_be_bytes([plain[addr_end], plain[addr_end + 1]]);
    let mut cursor = addr_end + 2;

    if plain.len() < cursor + 2 {
        return Err(Error::Protocol(
            "ss: 2022 var header truncated padding length",
        ));
    }
    let padding_len = u16::from_be_bytes([plain[cursor], plain[cursor + 1]]) as usize;
    if padding_len > SS_2022_MAX_PADDING_LENGTH {
        return Err(Error::Protocol("ss: 2022 padding exceeds max"));
    }
    cursor += 2;
    if plain.len() < cursor + padding_len {
        return Err(Error::Protocol("ss: 2022 var header truncated padding"));
    }
    cursor += padding_len;

    let initial_payload = plain[cursor..].to_vec();
    // SIP022 3.1.3: reject if neither payload nor padding is present.
    if initial_payload.is_empty() && padding_len == 0 {
        return Err(Error::Protocol(
            "ss: 2022 var header needs payload or padding",
        ));
    }
    Ok((addr, port, initial_payload))
}

/// Build the response fixed-length header plaintext.
///
/// Layout: `type(1) + timestamp(8 BE) + request_salt(16/32) + length(2 BE)`.
/// `length` is the plaintext length of the first payload chunk that follows.
pub fn build_2022_response_fixed_header(
    timestamp: u64,
    request_salt: &[u8],
    length: u16,
) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::with_capacity(1 + 8 + request_salt.len() + 2);
    buf.push(SS_2022_HEADER_TYPE_SERVER_STREAM);
    buf.extend_from_slice(&timestamp.to_be_bytes());
    buf.extend_from_slice(request_salt);
    buf.extend_from_slice(&length.to_be_bytes());
    Ok(buf)
}

/// Plaintext length of the response fixed-length header for a given key size.
pub const fn ss_2022_response_header_plain_len(salt_len: usize) -> usize {
    1 + 8 + salt_len + 2
}

/// Parse the response fixed-length header plaintext.
///
/// Returns `(type, timestamp, request_salt, length)`. Validates the type byte.
pub fn parse_2022_response_fixed_header(
    plain: &[u8],
    salt_len: usize,
) -> Result<(u8, u64, Vec<u8>, u16), Error> {
    let need = ss_2022_response_header_plain_len(salt_len);
    if plain.len() != need {
        return Err(Error::Protocol("ss: 2022 response header bad length"));
    }
    if plain[0] != SS_2022_HEADER_TYPE_SERVER_STREAM {
        return Err(Error::Protocol("ss: 2022 response header bad type"));
    }
    let mut ts = [0u8; 8];
    ts.copy_from_slice(&plain[1..9]);
    let timestamp = u64::from_be_bytes(ts);
    let request_salt = plain[9..9 + salt_len].to_vec();
    let length = u16::from_be_bytes([plain[9 + salt_len], plain[9 + salt_len + 1]]);
    Ok((plain[0], timestamp, request_salt, length))
}

/// Encrypt a single 2022 AEAD chunk (one nonce increment).
///
/// Unlike [`encrypt_tcp_chunk`] which emits a length+payload pair (two
/// operations), this performs exactly one AEAD seal and advances the nonce
/// counter by one. Used for the 2022 request/response header chunks.
#[cfg(feature = "crypto")]
pub fn encrypt_tcp_2022_single_chunk(
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    let nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    aead_encrypt(cipher, key, &nonce, plaintext)
}

/// Decrypt a single 2022 AEAD chunk (one nonce increment). Inverse of
/// [`encrypt_tcp_2022_single_chunk`].
#[cfg(feature = "crypto")]
pub fn decrypt_tcp_2022_single_chunk(
    cipher: CipherKind,
    key: &[u8],
    nonce_counter: &mut u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    let nonce = tcp_nonce(*nonce_counter);
    *nonce_counter = nonce_counter.saturating_add(1);
    aead_decrypt(cipher, key, &nonce, ciphertext)
}

/// Generate random-length padding for a 2022 request header.
///
/// Returns padding of random length in `[0, SS_2022_MAX_PADDING_LENGTH]`. When
/// `ensure_nonzero` is true (no initial payload available), the length is
/// forced to at least 1, as required by SIP022 3.1.3.
#[cfg(all(feature = "crypto", feature = "blake3"))]
pub fn random_2022_padding(ensure_nonzero: bool) -> Result<Vec<u8>, Error> {
    let mut len_bytes = [0u8; 2];
    fill_random(&mut len_bytes)?;
    let mut len = (u16::from_be_bytes(len_bytes) as usize) % (SS_2022_MAX_PADDING_LENGTH + 1);
    if ensure_nonzero && len == 0 {
        len = 1;
    }
    let mut padding = vec![0u8; len];
    fill_random(&mut padding)?;
    Ok(padding)
}

/// Validate a 2022 header timestamp against the current system time.
///
/// SIP022 3.1.5: messages with over 30 seconds of skew are treated as replay.
#[cfg(all(feature = "crypto", feature = "blake3"))]
pub fn validate_2022_timestamp(timestamp: u64) -> Result<(), Error> {
    let now = now_unix_seconds();
    let diff = now
        .saturating_sub(timestamp)
        .max(timestamp.saturating_sub(now));
    if diff > SS_2022_TIMESTAMP_WINDOW_SECS {
        return Err(Error::Protocol("ss: 2022 timestamp outside replay window"));
    }
    Ok(())
}

/// Server-side replay protection for Shadowsocks 2022 (SIP022 3.1.5).
///
/// Stores every accepted request salt for a rolling window and rejects a
/// salt that has been seen within the window. The timestamp check (30 s) is
/// the primary replay filter; this pool defends against replays inside that
/// window. Bloom filters and other false-positive structures are forbidden by
/// the spec, so an exact `HashMap` with per-entry timestamps is used.
///
/// One pool should be shared across all connections of a single SS listener so
/// a replayed salt is caught regardless of which connection it arrives on.
#[cfg(all(feature = "crypto", feature = "blake3"))]
pub struct ReplaySaltPool {
    inner: std::sync::Mutex<std::collections::HashMap<Vec<u8>, std::time::Instant>>,
    ttl: std::time::Duration,
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
impl ReplaySaltPool {
    /// Default window per SIP022 3.1.5 ("at least 60 seconds").
    pub const DEFAULT_TTL: std::time::Duration = std::time::Duration::from_secs(60);

    pub fn new() -> Self {
        Self::new_with_ttl(Self::DEFAULT_TTL)
    }

    /// Construct with an explicit TTL (primarily for tests).
    pub fn new_with_ttl(ttl: std::time::Duration) -> Self {
        Self {
            inner: std::sync::Mutex::new(std::collections::HashMap::new()),
            ttl,
        }
    }

    /// Validate that `salt` is fresh, then record it.
    ///
    /// Evicts expired entries first, then returns an error if the salt is
    /// already present (replay). Otherwise inserts it.
    pub fn check_and_insert(&self, salt: &[u8]) -> Result<(), Error> {
        let now = std::time::Instant::now();
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| Error::Protocol("ss: 2022 salt pool poisoned"))?;
        inner.retain(|_, observed| now.duration_since(*observed) < self.ttl);
        if inner.contains_key(salt) {
            return Err(Error::Protocol("ss: 2022 replay salt rejected"));
        }
        inner.insert(salt.to_vec(), now);
        Ok(())
    }
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
impl Default for ReplaySaltPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
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

#[cfg(all(feature = "crypto", feature = "blake3"))]
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

#[cfg(all(feature = "crypto", feature = "blake3"))]
fn random_u64() -> Result<u64, Error> {
    let mut bytes = [0u8; 8];
    fill_random(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
fn fill_random(bytes: &mut [u8]) -> Result<(), Error> {
    use ring::rand::SecureRandom;
    ring::rand::SystemRandom::new()
        .fill(bytes)
        .map_err(|_| Error::Protocol("ss: random failed"))
}

#[cfg(all(feature = "crypto", feature = "blake3"))]
pub fn now_unix_seconds() -> u64 {
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
    if payload_len > max_tcp_payload_len(cipher) {
        return Err(Error::Protocol("ss: tcp chunk too large"));
    }
    Ok(payload_len)
}

/// Maximum payload length per chunk. Legacy AEAD caps at 0x3FFF; SIP022 2022
/// removes that cap and allows up to 0xFFFF (spec 3.1.2).
pub const fn max_tcp_payload_len(cipher: CipherKind) -> usize {
    if cipher.is_blake3() {
        0xFFFF
    } else {
        MAX_TCP_PAYLOAD_SIZE
    }
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
