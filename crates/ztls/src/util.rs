//! Utility functions for TLS 1.3 message parsing.
//!
//! Extracted from REALITY's reality_util.rs.

/// Extract cipher suite ID from ServerHello plaintext.
pub fn extract_server_cipher_suite(data: &[u8]) -> std::io::Result<u16> {
    if data.len() < 40 {
        return Err(std::io::Error::other(
            "server hello too short for cipher suite",
        ));
    }
    // In a ServerHello, cipher suite is at offset 38 (handshake header + version + random + session_id_len + session_id)
    // Actually the offset depends on the session_id length.
    // For a standard ServerHello without session ID, cipher suite is at offset 38.
    let session_id_len = if data.len() > 38 {
        data[38] as usize
    } else {
        0
    };
    let cs_offset = 39 + session_id_len;
    if data.len() < cs_offset + 2 {
        return Err(std::io::Error::other(
            "server hello too short for cipher suite",
        ));
    }
    Ok(u16::from_be_bytes([data[cs_offset], data[cs_offset + 1]]))
}

/// Extract server public key from ServerHello key_share extension.
pub fn extract_server_public_key(data: &[u8]) -> std::io::Result<Vec<u8>> {
    if let Some(pos) = data.windows(2).position(|w| w == [0x00, 0x33]) {
        if data.len() > pos + 6 {
            let key_len = u16::from_be_bytes([data[pos + 4], data[pos + 5]]) as usize;
            if data.len() >= pos + 6 + key_len {
                return Ok(data[pos + 6..pos + 6 + key_len].to_vec());
            }
        }
    }
    Err(std::io::Error::other("key_share extension not found"))
}

// ── Server-side helpers (used by REALITY server) ──

/// Extract client public key from ClientHello key_share extension.
pub fn extract_client_public_key(data: &[u8]) -> std::io::Result<Vec<u8>> {
    if let Some(pos) = data.windows(2).position(|w| w == [0x00, 0x33]) {
        if data.len() > pos + 8 {
            let key_len = u16::from_be_bytes([data[pos + 6], data[pos + 7]]) as usize;
            if data.len() >= pos + 8 + key_len {
                return Ok(data[pos + 8..pos + 8 + key_len].to_vec());
            }
        }
    }
    Err(std::io::Error::other("client key_share not found"))
}

/// Extract cipher suites from ClientHello.
pub fn extract_client_cipher_suites(data: &[u8]) -> std::io::Result<Vec<u16>> {
    // ClientHello: offset 38 is session_id_length, then session_id, then cipher suites
    if data.len() < 40 {
        return Ok(Vec::new());
    }
    let sid_len = data[38] as usize;
    let cs_offset = 39 + sid_len;
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

/// Extract client random from ClientHello.
pub fn extract_client_random(data: &[u8]) -> std::io::Result<[u8; 32]> {
    if data.len() < 38 {
        return Err(std::io::Error::other("client hello too short"));
    }
    let mut random = [0u8; 32];
    random.copy_from_slice(&data[6..38]);
    Ok(random)
}

/// Extract session ID from ClientHello.
pub fn extract_session_id_slice<'a>(data: &'a [u8]) -> std::io::Result<&'a [u8]> {
    if data.len() < 40 {
        return Err(std::io::Error::other("client hello too short"));
    }
    let sid_len = data[38] as usize;
    if data.len() < 39 + sid_len {
        return Err(std::io::Error::other("session id truncated"));
    }
    Ok(&data[39..39 + sid_len])
}

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
