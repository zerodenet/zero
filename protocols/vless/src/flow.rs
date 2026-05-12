// VLESS AEAD flow implementation (xtls-rprx-vision compatible)
//
// When a flow is configured, the VLESS request header (command + port +
// address) is encrypted with AES-128-GCM using a key derived from the UUID.
// This provides additional obfuscation on top of any transport-layer TLS.

use alloc::vec::Vec;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_128_GCM};
use ring::digest;
use ring::hkdf;
use ring::rand::SecureRandom;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::shared::{read_exact, write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6};

pub const FLOW_XTLS_RPRX_VISION: &str = "xtls-rprx-vision";
pub const FLOW_XTLS_RPRX_VISION_UDP: &str = "xtls-rprx-vision-udp443";

const AEAD_KEY_LEN: usize = 16;
const AEAD_NONCE_LEN: usize = 12;
const AEAD_TAG_LEN: usize = 16;

// ——— flow metadata ———

pub fn parse_flow(name: &str) -> Result<&'static str, Error> {
    match name {
        FLOW_XTLS_RPRX_VISION => Ok(FLOW_XTLS_RPRX_VISION),
        FLOW_XTLS_RPRX_VISION_UDP => Ok(FLOW_XTLS_RPRX_VISION_UDP),
        _ => Err(Error::Unsupported("VLESS flow is not supported")),
    }
}

pub fn flow_byte(flow: Option<&str>) -> u8 {
    match flow {
        Some(FLOW_XTLS_RPRX_VISION | FLOW_XTLS_RPRX_VISION_UDP) => 0x01,
        _ => 0x00,
    }
}

pub fn flow_from_byte(byte: u8) -> Option<&'static str> {
    match byte {
        0x01 => Some(FLOW_XTLS_RPRX_VISION),
        _ => None,
    }
}

pub fn is_aead_flow(flow: Option<&str>) -> bool {
    flow == Some(FLOW_XTLS_RPRX_VISION) || flow == Some(FLOW_XTLS_RPRX_VISION_UDP)
}

// ——— key derivation ———

fn derive_flow_key(uuid: &[u8; 16], salt: &[u8]) -> Result<[u8; AEAD_KEY_LEN], Error> {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, salt);
    let prk = salt.extract(uuid);
    let info = b"vless flow aead key";
    let mut key = [0u8; AEAD_KEY_LEN];
    prk.expand(&[info], HKDFKeyLen(AEAD_KEY_LEN))
        .and_then(|okm| okm.fill(&mut key))
        .map_err(|_| Error::Protocol("flow key derivation failed"))?;
    Ok(key)
}

struct HKDFKeyLen(usize);

impl hkdf::KeyType for HKDFKeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

// ——— outbound: encrypt command block ———

/// Encrypt the command block (command + port + address) for AEAD flows.
///
/// Returns the flow byte and the encrypted payload.
/// Encrypted payload format: [random_padding_len(1)] [padding(var)] [plain_block_len(2)] [plain_block] + tag
pub fn flow_build_request(
    uuid: &[u8; 16],
    flow: Option<&str>,
    command: u8,
    port: u16,
    address: &Address,
) -> Result<(u8, Vec<u8>), Error> {
    if !is_aead_flow(flow) {
        // Plain mode: flow byte + command + port + address
        let mut buf = Vec::with_capacity(4);
        buf.push(command);
        buf.extend_from_slice(&port.to_be_bytes());
        write_address(&mut buf, address)?;
        return Ok((0x00, buf));
    }

    // Build plain block
    let mut plain = Vec::new();
    plain.push(command);
    plain.extend_from_slice(&port.to_be_bytes());
    write_address(&mut plain, address)?;

    // Generate random salt (sent as part of the encrypted block)
    let rng = ring::rand::SystemRandom::new();
    let mut salt = [0u8; 8];
    rng.fill(&mut salt)
        .map_err(|_| Error::Protocol("random generation failed"))?;

    let key_bytes = derive_flow_key(uuid, &salt)?;
    let unbound = UnboundKey::new(&AES_128_GCM, &key_bytes)
        .map_err(|_| Error::Protocol("flow key init failed"))?;
    let key = LessSafeKey::new(unbound);

    // Random padding (0-31 bytes) for length obfuscation
    let mut pad_buf = [0u8; 1];
    rng.fill(&mut pad_buf)
        .map_err(|_| Error::Protocol("random generation failed"))?;
    let pad_len: usize = (pad_buf[0] & 0x1F) as usize;
    let mut padded = Vec::with_capacity(1 + pad_len + 2 + plain.len() + AEAD_TAG_LEN);
    padded.push(pad_len as u8);
    padded.extend_from_slice(&vec![0u8; pad_len]);
    padded.extend_from_slice(&(plain.len() as u16).to_be_bytes());
    padded.extend_from_slice(&plain);

    // Generate nonce from salt (first 12 bytes of SHA256(salt || "nonce"))
    let mut nonce_bytes = [0u8; AEAD_NONCE_LEN];
    let mut ctx = digest::Context::new(&digest::SHA256);
    ctx.update(&salt);
    ctx.update(b"vless flow nonce");
    let hash = ctx.finish();
    nonce_bytes.copy_from_slice(&hash.as_ref()[..AEAD_NONCE_LEN]);

    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut padded)
        .map_err(|_| Error::Protocol("flow encryption failed"))?;

    // Prepend salt so the receiver can derive the key
    let mut payload = Vec::with_capacity(salt.len() + padded.len());
    payload.extend_from_slice(&salt);
    payload.extend_from_slice(&padded);

    Ok((0x01, payload))
}

// ——— inbound: decrypt command block ———

/// Decrypt the command block for AEAD flows.
///
/// Returns (command, port, address).
pub async fn flow_read_request<S>(
    stream: &mut S,
    flow: Option<&str>,
    uuid: &[u8; 16],
) -> Result<(u8, u16, Address), Error>
where
    S: AsyncSocket,
{
    if !is_aead_flow(flow) {
        // Plain mode
        let mut command = [0u8; 1];
        read_exact(stream, &mut command).await?;
        let mut port = [0u8; 2];
        read_exact(stream, &mut port).await?;
        let port = u16::from_be_bytes(port);
        if port == 0 {
            return Err(Error::Protocol("VLESS target port must not be 0"));
        }
        let mut atyp = [0u8; 1];
        read_exact(stream, &mut atyp).await?;
        let target = read_address_from_stream(stream, atyp[0]).await?;
        return Ok((command[0], port, target));
    }

    // AEAD mode: read salt + encrypted payload
    let mut salt = [0u8; 8];
    read_exact(stream, &mut salt).await?;

    // Determine payload length: read in chunks looking for complete AEAD record
    // AEAD payload = 1(pad_len) + pad_len + 2(plain_len) + plain + 16(tag)
    // Minimum: 1 + 0 + 2 + 0 + 16 = 19, Maximum: 1 + 31 + 2 + 255 + 16 = 305
    let mut buf = Vec::with_capacity(320);
    let mut total_read = 0usize;

    // First read enough to get pad_len (1 byte)
    ensure_read(stream, &mut buf, 1).await?;
    total_read = buf.len();

    let pad_len = buf[0] as usize;
    if pad_len > 31 {
        return Err(Error::Protocol("VLESS flow: invalid padding length"));
    }

    // Ensure we have pad_len + 2(plain_len) more bytes
    let header_needed = pad_len + 2;
    if buf.len() < 1 + header_needed {
        ensure_read(stream, &mut buf, 1 + header_needed).await?;
        total_read = buf.len();
    }

    let plain_len = u16::from_be_bytes([buf[1 + pad_len], buf[1 + pad_len + 1]]) as usize;
    if plain_len > 300 {
        return Err(Error::Protocol("VLESS flow: invalid plain block length"));
    }

    // Now we know the total AEAD payload size: 1 + pad_len + 2 + plain_len + 16(tag)
    let total_payload = 1 + pad_len + 2 + plain_len + AEAD_TAG_LEN;
    if buf.len() < total_payload {
        ensure_read(stream, &mut buf, total_payload).await?;
    }
    let mut encrypted = buf[..total_payload].to_vec();

    // Derive key and decrypt
    let key_bytes = derive_flow_key(uuid, &salt)?;
    let unbound = UnboundKey::new(&AES_128_GCM, &key_bytes)
        .map_err(|_| Error::Protocol("flow key init failed"))?;
    let key = LessSafeKey::new(unbound);

    let mut nonce_bytes = [0u8; AEAD_NONCE_LEN];
    let mut ctx = digest::Context::new(&digest::SHA256);
    ctx.update(&salt);
    ctx.update(b"vless flow nonce");
    let hash = ctx.finish();
    nonce_bytes.copy_from_slice(&hash.as_ref()[..AEAD_NONCE_LEN]);

    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let decrypted = key
        .open_in_place(nonce, Aad::empty(), &mut encrypted)
        .map_err(|_| {
            Error::Protocol("VLESS flow decryption failed - wrong key or corrupted data")
        })?;

    // Parse decrypted: pad_len(1) + padding(pad_len) + plain_len(2) + plain
    let plain_offset = 1 + pad_len + 2;
    let plain = &decrypted[plain_offset..plain_offset + plain_len];
    if plain.is_empty() {
        return Err(Error::Protocol("VLESS flow: empty plain block"));
    }

    let command = plain[0];
    let port = u16::from_be_bytes([plain[1], plain[2]]);
    if port == 0 {
        return Err(Error::Protocol("VLESS target port must not be 0"));
    }
    let atyp = plain[3];

    let target = read_address_from_bytes(atyp, &plain[4..])?;

    Ok((command, port, target))
}

async fn ensure_read<S>(stream: &mut S, buf: &mut Vec<u8>, target_len: usize) -> Result<(), Error>
where
    S: AsyncSocket,
{
    while buf.len() < target_len {
        let mut chunk = [0u8; 512];
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|_| Error::Io("failed to read flow data"))?;
        if n == 0 {
            return Err(Error::Io("unexpected EOF during flow read"));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(())
}

fn read_address_from_bytes(atyp: u8, data: &[u8]) -> Result<Address, Error> {
    match atyp {
        ATYP_IPV4 => {
            if data.len() < 4 {
                return Err(Error::Protocol("VLESS flow: truncated IPv4 address"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[..4]);
            Ok(Address::Ipv4(bytes))
        }
        ATYP_IPV6 => {
            if data.len() < 16 {
                return Err(Error::Protocol("VLESS flow: truncated IPv6 address"));
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[..16]);
            Ok(Address::Ipv6(bytes))
        }
        ATYP_DOMAIN => {
            if data.is_empty() {
                return Err(Error::Protocol("VLESS flow: truncated domain address"));
            }
            let len = data[0] as usize;
            if len == 0 || data.len() < 1 + len {
                return Err(Error::Protocol("VLESS flow: truncated domain address"));
            }
            let domain = alloc::string::String::from_utf8(data[1..1 + len].to_vec())
                .map_err(|_| Error::Protocol("VLESS domain is not valid UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        _ => Err(Error::Unsupported("VLESS address type is not supported")),
    }
}

async fn read_address_from_stream<S>(stream: &mut S, atyp: u8) -> Result<Address, Error>
where
    S: AsyncSocket,
{
    match atyp {
        ATYP_IPV4 => {
            let mut bytes = [0u8; 4];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv4(bytes))
        }
        ATYP_IPV6 => {
            let mut bytes = [0u8; 16];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv6(bytes))
        }
        ATYP_DOMAIN => {
            let mut length = [0u8; 1];
            read_exact(stream, &mut length).await?;
            let len = length[0] as usize;
            if len == 0 {
                return Err(Error::Protocol("VLESS domain must not be empty"));
            }
            let mut domain = alloc::vec![0u8; len];
            read_exact(stream, &mut domain).await?;
            let domain = alloc::string::String::from_utf8(domain)
                .map_err(|_| Error::Protocol("VLESS domain is not valid UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        _ => Err(Error::Unsupported("VLESS address type is not supported")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::parse_uuid;

    #[test]
    fn test_flow_roundtrip() {
        let uuid_str = "b831381d-6324-4d53-ad4f-8cda48b30811";
        let uuid = parse_uuid(uuid_str).unwrap();
        let flow = Some(FLOW_XTLS_RPRX_VISION);

        let address = Address::Domain("example.com".into());
        let (fbyte, payload) = flow_build_request(&uuid, flow, 0x01, 443, &address).unwrap();

        assert_eq!(fbyte, 0x01);
        // salt(8) + padded_encrypted: pad_len(1) + padding + plain_len(2) + plain + tag(16)
        assert!(payload.len() >= 8 + 1 + 2 + 16);
    }

    #[test]
    fn test_plain_no_flow() {
        let uuid_str = "b831381d-6324-4d53-ad4f-8cda48b30811";
        let uuid = parse_uuid(uuid_str).unwrap();

        let address = Address::Ipv4([127, 0, 0, 1]);
        let (fbyte, payload) = flow_build_request(&uuid, None, 0x01, 80, &address).unwrap();

        assert_eq!(fbyte, 0x00);
        // Plain: command(1) + port(2) + atyp(1) + ipv4(4) = 8
        assert_eq!(payload.len(), 8);
        assert_eq!(payload[0], 0x01); // TCP
        assert_eq!(u16::from_be_bytes([payload[1], payload[2]]), 80);
    }

    #[test]
    fn test_parse_flow_valid() {
        assert!(parse_flow(FLOW_XTLS_RPRX_VISION).is_ok());
        assert!(parse_flow(FLOW_XTLS_RPRX_VISION_UDP).is_ok());
    }

    #[test]
    fn test_parse_flow_invalid() {
        assert!(parse_flow("unknown-flow").is_err());
        assert!(parse_flow("").is_err());
    }

    #[test]
    fn test_flow_byte_mapping() {
        assert_eq!(flow_byte(Some(FLOW_XTLS_RPRX_VISION)), 0x01);
        assert_eq!(flow_byte(None), 0x00);
        assert_eq!(flow_from_byte(0x01), Some(FLOW_XTLS_RPRX_VISION));
        assert_eq!(flow_from_byte(0x00), None);
        assert_eq!(flow_from_byte(0xFF), None);
    }
}
