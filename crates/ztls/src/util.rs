//! Utility functions for TLS 1.3 message parsing.
//!
//! All functions expect **full** TLS records (including the 5-byte record
//! header).  This matches how the callers in `reality_server_connection` and
//! `reality_client_connection` pass data.

use crate::buf_reader::BufReader;

// ── Offsets within a full TLS record ──────────────────────────────────

const TLS_HEADER_LEN: usize = 5;
const HANDSHAKE_HEADER_LEN: usize = 4; // type(1) + length(3)
const PROTOCOL_VERSION_LEN: usize = 2;
const RANDOM_LEN: usize = 32;
const RANDOM_OFFSET: usize = TLS_HEADER_LEN + HANDSHAKE_HEADER_LEN + PROTOCOL_VERSION_LEN; // 11
const SID_LEN_OFFSET: usize = RANDOM_OFFSET + RANDOM_LEN; // 43

// ── ClientHello helpers ──────────────────────────────────────────────

/// Extract client random from a full TLS ClientHello record.
pub fn extract_client_random(data: &[u8]) -> std::io::Result<[u8; 32]> {
    if data.len() < RANDOM_OFFSET + 32 {
        return Err(std::io::Error::other("client hello too short"));
    }
    let mut random = [0u8; 32];
    random.copy_from_slice(&data[RANDOM_OFFSET..RANDOM_OFFSET + 32]);
    Ok(random)
}

/// Extract session ID slice from a full TLS ClientHello record.
pub fn extract_session_id_slice<'a>(data: &'a [u8]) -> std::io::Result<&'a [u8]> {
    if data.len() < SID_LEN_OFFSET + 1 {
        return Err(std::io::Error::other("client hello too short"));
    }
    let sid_len = data[SID_LEN_OFFSET] as usize;
    if data.len() < SID_LEN_OFFSET + 1 + sid_len {
        return Err(std::io::Error::other("session id truncated"));
    }
    Ok(&data[SID_LEN_OFFSET + 1..SID_LEN_OFFSET + 1 + sid_len])
}

/// Extract cipher suites from a full TLS ClientHello record.
pub fn extract_client_cipher_suites(data: &[u8]) -> std::io::Result<Vec<u16>> {
    if data.len() < SID_LEN_OFFSET + 1 {
        return Ok(Vec::new());
    }
    let sid_len = data[SID_LEN_OFFSET] as usize;
    let cs_offset = SID_LEN_OFFSET + 1 + sid_len;
    if data.len() < cs_offset + 2 {
        return Ok(Vec::new());
    }
    let cs_len = u16::from_be_bytes([data[cs_offset], data[cs_offset + 1]]) as usize;
    let mut suites = Vec::new();
    for i in 0..cs_len / 2 {
        if data.len() >= cs_offset + 2 + (i + 1) * 2 {
            suites.push(u16::from_be_bytes([
                data[cs_offset + 2 + i * 2],
                data[cs_offset + 2 + i * 2 + 1],
            ]));
        }
    }
    Ok(suites)
}

/// Extract client public key (X25519) from ClientHello key_share extension.
///
/// Uses structured parsing to correctly locate the extensions section,
/// then finds the key_share extension (type 0x0033) and extracts the
/// X25519 public key.
pub fn extract_client_public_key(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut reader = BufReader::new(data);
    reader.skip(TLS_HEADER_LEN)?;

    let _handshake_type = reader.read_u8()?;
    let _handshake_len = reader.read_u24_be()?;
    let _version = reader.read_u16_be()?;
    reader.skip(32)?; // random

    let sid_len = reader.read_u8()? as usize;
    reader.skip(sid_len)?;

    let cs_len = reader.read_u16_be()? as usize;
    reader.skip(cs_len)?;

    let comp_len = reader.read_u8()? as usize;
    reader.skip(comp_len)?;

    parse_keyshare_from_extensions(&mut reader)
}

// ── ServerHello helpers ──────────────────────────────────────────────

/// Extract cipher suite ID from a full TLS ServerHello record.
pub fn extract_server_cipher_suite(data: &[u8]) -> std::io::Result<u16> {
    // ServerHello: [record_hdr:5][hs_type:1][length:3][version:2][random:32][sid_len:1][sid...][cipher_suite:2]
    const SERVER_SID_LEN_OFFSET: usize = TLS_HEADER_LEN + 1 + 3 + PROTOCOL_VERSION_LEN + RANDOM_LEN;
    if data.len() < SERVER_SID_LEN_OFFSET + 1 {
        return Err(std::io::Error::other("server hello too short for cipher suite"));
    }
    let session_id_len = data[SERVER_SID_LEN_OFFSET] as usize;
    let cs_offset = SERVER_SID_LEN_OFFSET + 1 + session_id_len;
    if data.len() < cs_offset + 2 {
        return Err(std::io::Error::other("server hello too short for cipher suite"));
    }
    Ok(u16::from_be_bytes([data[cs_offset], data[cs_offset + 1]]))
}

/// Extract server public key (X25519) from ServerHello key_share extension.
///
/// Uses structured parsing to skip through ServerHello fields and find the
/// key_share extension.  The ServerHello key_share has a flat structure
/// (no shares-length prefix unlike ClientHello).
pub fn extract_server_public_key(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut reader = BufReader::new(data);
    reader.skip(TLS_HEADER_LEN)?;

    let _handshake_type = reader.read_u8()?;
    let _handshake_len = reader.read_u24_be()?;
    let _version = reader.read_u16_be()?;
    reader.skip(32)?; // random

    let sid_len = reader.read_u8()? as usize;
    reader.skip(sid_len)?;

    reader.skip(2)?; // cipher suite
    reader.skip(1)?; // legacy_compression_method

    // Extensions
    let extensions_len = reader.read_u16_be()? as usize;
    let extensions_end = reader.position() + extensions_len;

    while reader.position() + 4 <= extensions_end {
        let ext_type = reader.read_u16_be()?;
        let ext_len = reader.read_u16_be()? as usize;

        if ext_type == 0x0033 {
            // ServerHello key_share: [group:2][key_len:2][key_data...]
            let group = reader.read_u16_be()?;
            let key_len = reader.read_u16_be()? as usize;
            if group == 0x001d {
                return Ok(reader.read_slice(key_len)?.to_vec());
            }
            return Err(std::io::Error::other("X25519 key share not found in ServerHello"));
        }
        reader.skip(ext_len)?;
    }

    Err(std::io::Error::other("key_share extension not found"))
}

// ── Shared extension parser ─────────────────────────────────────────

/// Walk the extensions list and extract the X25519 key from key_share (0x0033).
fn parse_keyshare_from_extensions(reader: &mut BufReader<'_>) -> std::io::Result<Vec<u8>> {
    let extensions_len = reader.read_u16_be()? as usize;
    let extensions_end = reader.position() + extensions_len;

    while reader.position() + 4 <= extensions_end {
        let ext_type = reader.read_u16_be()?;
        let ext_len = reader.read_u16_be()? as usize;

        if ext_type == 0x0033 {
            return parse_keyshare_entry(reader.read_slice(ext_len)?);
        }
        reader.skip(ext_len)?;
    }

    Err(std::io::Error::other("key_share extension not found"))
}

/// Parse a key_share extension payload, looking for X25519 (group 0x001d).
fn parse_keyshare_entry(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut reader = BufReader::new(data);
    let _client_shares_len = reader.read_u16_be()?;

    loop {
        if reader.position() + 4 > data.len() {
            break;
        }
        let group = reader.read_u16_be()?;
        let key_len = reader.read_u16_be()? as usize;

        if reader.position() + key_len > data.len() {
            return Err(std::io::Error::other("KeyShare entry extends past end"));
        }

        if group == 0x001d {
            // X25519
            return Ok(reader.read_slice(key_len)?.to_vec());
        }
        reader.skip(key_len)?;
    }

    Err(std::io::Error::other("X25519 key share not found"))
}

// ── Cipher suite negotiation ─────────────────────────────────────────

/// Select matching cipher suite between client and server.
pub fn negotiate_cipher_suite(
    client_suites: &[u16],
    server_suites: &[u16],
) -> std::io::Result<u16> {
    for cs in client_suites {
        if server_suites.contains(cs) {
            return Ok(*cs);
        }
    }
    Err(std::io::Error::other("no common cipher suite"))
}
