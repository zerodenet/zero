// TLS 1.3 Message Construction
//
// Construct TLS 1.3 handshake messages for REALITY protocol

use super::common::{HANDSHAKE_TYPE_FINISHED, VERSION_TLS_1_2_MAJOR, VERSION_TLS_1_2_MINOR};
use std::io::Result;

/// Construct Finished message
///
/// # Arguments
/// * `verify_data` - HMAC of handshake transcript (32 bytes for SHA256)
pub fn construct_finished(verify_data: &[u8]) -> Result<Vec<u8>> {
    let mut finished = Vec::new();

    // Finished structure:
    // - handshake_type (1 byte) = 20
    // - length (3 bytes)
    // - verify_data (variable, 32 bytes for SHA256)

    finished.push(HANDSHAKE_TYPE_FINISHED);

    // Payload length (3 bytes)
    finished.extend_from_slice(&[
        ((verify_data.len() >> 16) & 0xff) as u8,
        ((verify_data.len() >> 8) & 0xff) as u8,
        (verify_data.len() & 0xff) as u8,
    ]);

    finished.extend_from_slice(verify_data);

    Ok(finished)
}

/// Default ALPN protocols for REALITY client (matches browser fingerprints)
pub const DEFAULT_ALPN_PROTOCOLS: &[&str] = &["h2", "http/1.1"];

/// Construct TLS 1.3 ClientHello message
///
/// Returns handshake message bytes (without record header)
///
/// # Arguments
/// * `client_random` - 32 bytes client random
/// * `session_id` - 32 bytes session ID
/// * `client_public_key` - X25519 public key bytes
/// * `server_name` - SNI hostname
/// * `cipher_suites` - Cipher suite IDs to offer (e.g., &[0x1301, 0x1302, 0x1303])
/// * `alpn_protocols` - ALPN protocols to offer (e.g., &["h2", "http/1.1"])
pub fn construct_client_hello(
    client_random: &[u8; 32],
    session_id: &[u8; 32],
    client_public_key: &[u8],
    server_name: &str,
    cipher_suites: &[u16],
    alpn_protocols: &[&str],
) -> Result<Vec<u8>> {
    let mut hello = Vec::with_capacity(512);

    // Handshake message type: ClientHello (0x01)
    hello.push(0x01);

    // Placeholder for handshake message length (3 bytes)
    let length_offset = hello.len();
    hello.extend_from_slice(&[0u8; 3]);

    // TLS version: 3.3 (TLS 1.2 for compatibility)
    hello.extend_from_slice(&[VERSION_TLS_1_2_MAJOR, VERSION_TLS_1_2_MINOR]);

    // Client random (32 bytes)
    hello.extend_from_slice(client_random);

    // Session ID length (1 byte) + Session ID (32 bytes)
    hello.push(32);
    hello.extend_from_slice(session_id);

    // Cipher suites
    let cipher_suites_len = (cipher_suites.len() * 2) as u16;
    hello.extend_from_slice(&cipher_suites_len.to_be_bytes());
    for &suite in cipher_suites {
        hello.extend_from_slice(&suite.to_be_bytes());
    }

    // Compression methods (1 method: null)
    hello.extend_from_slice(&[0x01, 0x00]);

    // Extensions
    let extensions_offset = hello.len();
    hello.extend_from_slice(&[0u8; 2]); // Placeholder for extensions length

    let mut extensions = Vec::new();

    // server_name extension (type 0)
    {
        let server_name_bytes = server_name.as_bytes();
        let server_name_len = server_name_bytes.len();

        extensions.extend_from_slice(&[0x00, 0x00]); // Extension type: server_name
        let ext_len = 5 + server_name_len;
        extensions.extend_from_slice(&(ext_len as u16).to_be_bytes()); // Extension length
        extensions.extend_from_slice(&((server_name_len + 3) as u16).to_be_bytes()); // Server name list length
        extensions.push(0x00); // Name type: host_name
        extensions.extend_from_slice(&(server_name_len as u16).to_be_bytes()); // Name length
        extensions.extend_from_slice(server_name_bytes); // Server name
    }

    // supported_versions extension (type 43)
    {
        extensions.extend_from_slice(&[0x00, 0x2b]); // Extension type: supported_versions
        extensions.extend_from_slice(&[0x00, 0x03]); // Extension length: 3
        extensions.push(0x02); // Supported versions length: 2
        extensions.extend_from_slice(&[0x03, 0x04]); // TLS 1.3
    }

    // supported_groups extension (type 10)
    {
        extensions.extend_from_slice(&[0x00, 0x0a]); // Extension type: supported_groups
        extensions.extend_from_slice(&[0x00, 0x04]); // Extension length: 4
        extensions.extend_from_slice(&[0x00, 0x02]); // Supported groups length: 2
        extensions.extend_from_slice(&[0x00, 0x1d]); // x25519
    }

    // key_share extension (type 51)
    {
        extensions.extend_from_slice(&[0x00, 0x33]); // Extension type: key_share
        let key_share_len = 2 + 4 + client_public_key.len();
        extensions.extend_from_slice(&(key_share_len as u16).to_be_bytes()); // Extension length
        let key_share_list_len = 4 + client_public_key.len();
        extensions.extend_from_slice(&(key_share_list_len as u16).to_be_bytes()); // Key share list length
        extensions.extend_from_slice(&[0x00, 0x1d]); // Group: x25519
        extensions.extend_from_slice(&(client_public_key.len() as u16).to_be_bytes()); // Key length
        extensions.extend_from_slice(client_public_key); // Public key
    }

    // signature_algorithms extension (type 13)
    {
        extensions.extend_from_slice(&[0x00, 0x0d]); // Extension type: signature_algorithms
        extensions.extend_from_slice(&[0x00, 0x04]); // Extension length: 4
        extensions.extend_from_slice(&[0x00, 0x02]); // Signature algorithms length: 2
        extensions.extend_from_slice(&[0x08, 0x07]); // ed25519
    }

    // ALPN extension (type 16)
    if !alpn_protocols.is_empty() {
        extensions.extend_from_slice(&[0x00, 0x10]); // Extension type: ALPN (16)

        // Calculate total length of protocol list
        let protocols_list_len: usize = alpn_protocols
            .iter()
            .map(|p| 1 + p.len()) // 1 byte length prefix + protocol bytes
            .sum();

        // Extension length = 2 (list length field) + protocols_list_len
        let ext_len = 2 + protocols_list_len;
        extensions.extend_from_slice(&(ext_len as u16).to_be_bytes());

        // Protocol list length
        extensions.extend_from_slice(&(protocols_list_len as u16).to_be_bytes());

        // Each protocol: 1 byte length + protocol string
        for protocol in alpn_protocols {
            extensions.push(protocol.len() as u8);
            extensions.extend_from_slice(protocol.as_bytes());
        }
    }

    // Write extensions length
    let extensions_length = extensions.len();
    hello[extensions_offset..extensions_offset + 2]
        .copy_from_slice(&(extensions_length as u16).to_be_bytes());

    // Append extensions
    hello.extend_from_slice(&extensions);

    // Write handshake message length
    let message_length = hello.len() - 4; // Exclude type (1) and length (3)
    hello[length_offset..length_offset + 3]
        .copy_from_slice(&(message_length as u32).to_be_bytes()[1..]);

    Ok(hello)
}

/// Write TLS record header
///
/// # Arguments
/// * `record_type` - TLS record type (0x16 for Handshake, 0x17 for ApplicationData)
/// * `length` - Length of record payload
pub fn write_record_header(record_type: u8, length: u16) -> Vec<u8> {
    let mut header = Vec::new();
    header.push(record_type);
    header.extend_from_slice(&[VERSION_TLS_1_2_MAJOR, VERSION_TLS_1_2_MINOR]); // Version: TLS 1.2
    header.extend_from_slice(&length.to_be_bytes());
    header
}

#[cfg(test)]
mod tests {
    use super::super::common::CONTENT_TYPE_HANDSHAKE;
    use super::*;

    #[test]
    fn test_construct_finished() {
        let verify_data = vec![0xCCu8; 32];
        let result = construct_finished(&verify_data);
        assert!(result.is_ok());
        let msg = result.unwrap();
        assert_eq!(msg[0], HANDSHAKE_TYPE_FINISHED);
        assert_eq!(msg.len(), 1 + 3 + 32); // type + length + verify_data
    }

    #[test]
    fn test_write_record_header() {
        let header = write_record_header(CONTENT_TYPE_HANDSHAKE, 100);
        assert_eq!(header.len(), 5);
        assert_eq!(header[0], 0x16); // Handshake
        assert_eq!(header[1], 0x03); // TLS 1.2
        assert_eq!(header[2], 0x03);
        assert_eq!(u16::from_be_bytes([header[3], header[4]]), 100);
    }
}
