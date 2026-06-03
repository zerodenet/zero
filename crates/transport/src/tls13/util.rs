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
    // Look for key_share extension (0x0033) in the ServerHello
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
