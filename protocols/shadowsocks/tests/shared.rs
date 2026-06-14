//! Tests for Shadowsocks protocol constants and helpers.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/shadowsocks/src/shared.rs`.
//!
//! The AEAD/TCP-chunk round-trips require the `crypto` feature; the
//! cipher/address/target tests run unconditionally.

use shadowsocks::{
    build_2022_request_fixed_header, build_2022_request_var_header,
    build_2022_response_fixed_header, build_target_data, decode_address, encode_address,
    parse_2022_request_fixed_header, parse_2022_request_var_header,
    parse_2022_response_fixed_header, parse_target_data, ss_2022_response_header_plain_len,
    CipherKind, SS_2022_HEADER_TYPE_CLIENT_STREAM, SS_2022_HEADER_TYPE_SERVER_STREAM,
    SS_2022_MAX_PADDING_LENGTH, SS_2022_REQUEST_FIXED_HEADER_LEN,
};
use zero_core::Address;

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
    use shadowsocks::{aead_decrypt, aead_encrypt, derive_key};

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
    use shadowsocks::{
        decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload, derive_key, encrypt_tcp_chunk,
        TCP_CHUNK_SIZE_LEN,
    };

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
        decrypt_tcp_chunk_length(cipher, &key, &mut decrypt_nonce, &chunk[..length_size]).unwrap();
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

// ---- SIP022 2022 edition TCP header byte layout ----

#[test]
fn ss_2022_request_fixed_header_byte_layout() {
    // type(1) + timestamp(8 BE) + length(2 BE) = 11 bytes.
    let header = build_2022_request_fixed_header(0x1122_3344_5566_7788, 0x1234);
    assert_eq!(header.len(), SS_2022_REQUEST_FIXED_HEADER_LEN);
    assert_eq!(header[0], SS_2022_HEADER_TYPE_CLIENT_STREAM);
    // Big-endian timestamp.
    assert_eq!(
        &header[1..9],
        &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
    );
    // Big-endian length.
    assert_eq!(&header[9..11], &[0x12, 0x34]);

    let (ty, ts, len) = parse_2022_request_fixed_header(&header).unwrap();
    assert_eq!(ty, SS_2022_HEADER_TYPE_CLIENT_STREAM);
    assert_eq!(ts, 0x1122_3344_5566_7788);
    assert_eq!(len, 0x1234);
}

#[test]
fn ss_2022_request_fixed_header_rejects_bad_length() {
    // 10 bytes instead of 11.
    assert!(parse_2022_request_fixed_header(&[0u8; 10]).is_err());
    // 12 bytes instead of 11.
    assert!(parse_2022_request_fixed_header(&[0u8; 12]).is_err());
}

#[test]
fn ss_2022_request_var_header_roundtrip_with_padding() {
    let padding = vec![0xaau8; 8];
    let var = build_2022_request_var_header(
        &Address::Domain("example.com".to_owned()),
        443,
        &padding,
        &[],
    )
    .unwrap();
    // ATYP(1) + len(1) + "example.com"(11) + port(2) + padding_len(2) + padding(8)
    assert_eq!(var.len(), 1 + 1 + 11 + 2 + 2 + 8);
    let (addr, port, payload) = parse_2022_request_var_header(&var).unwrap();
    assert_eq!(addr, Address::Domain("example.com".to_owned()));
    assert_eq!(port, 443);
    assert!(payload.is_empty(), "no initial payload carried");
}

#[test]
fn ss_2022_request_var_header_roundtrip_with_initial_payload() {
    let initial = b"GET / HTTP/1.1";
    let var = build_2022_request_var_header(&Address::Ipv4([93, 184, 216, 34]), 80, &[], initial)
        .unwrap();
    let (addr, port, payload) = parse_2022_request_var_header(&var).unwrap();
    assert_eq!(addr, Address::Ipv4([93, 184, 216, 34]));
    assert_eq!(port, 80);
    assert_eq!(payload, initial);
}

#[test]
fn ss_2022_request_var_header_rejects_no_payload_no_padding() {
    // Per SIP022 3.1.3: variable header with neither payload nor padding is
    // invalid. build_2022_request_var_header allows it (the caller controls
    // policy), but parse MUST reject it.
    let var = build_2022_request_var_header(&Address::Ipv4([1, 1, 1, 1]), 53, &[], &[]).unwrap();
    assert!(parse_2022_request_var_header(&var).is_err());
}

#[test]
fn ss_2022_request_var_header_rejects_oversized_padding() {
    let padding = vec![0u8; SS_2022_MAX_PADDING_LENGTH + 1];
    assert!(
        build_2022_request_var_header(&Address::Ipv4([1, 1, 1, 1]), 53, &padding, &[],).is_err()
    );
}

#[test]
fn ss_2022_response_header_byte_layout() {
    let request_salt = vec![0x55u8; 16];
    // type(1) + timestamp(8) + request_salt(16) + length(2) = 27 bytes plain.
    assert_eq!(ss_2022_response_header_plain_len(16), 27);
    assert_eq!(ss_2022_response_header_plain_len(32), 43);

    let header =
        build_2022_response_fixed_header(0x0011_2233_4455_6677, &request_salt, 0x7fff).unwrap();
    assert_eq!(header.len(), 27);
    assert_eq!(header[0], SS_2022_HEADER_TYPE_SERVER_STREAM);
    assert_eq!(
        &header[1..9],
        &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]
    );
    assert_eq!(&header[9..25], &request_salt[..]);
    assert_eq!(&header[25..27], &[0x7f, 0xff]);

    let (ty, ts, salt, len) = parse_2022_response_fixed_header(&header, 16).unwrap();
    assert_eq!(ty, SS_2022_HEADER_TYPE_SERVER_STREAM);
    assert_eq!(ts, 0x0011_2233_4455_6677);
    assert_eq!(salt, request_salt);
    assert_eq!(len, 0x7fff);
}

#[test]
fn ss_2022_response_header_rejects_wrong_type() {
    let salt = vec![0u8; 16];
    // Forge a response header with type 0 (client stream) instead of 1.
    let mut header = build_2022_response_fixed_header(1, &salt, 4).unwrap();
    header[0] = SS_2022_HEADER_TYPE_CLIENT_STREAM;
    assert!(parse_2022_response_fixed_header(&header, 16).is_err());
}

// ---- SIP022 3.1.5 server-side replay salt pool ----

#[cfg(feature = "blake3")]
#[test]
fn replay_salt_pool_accepts_distinct_salts() {
    use shadowsocks::ReplaySaltPool;
    let pool = ReplaySaltPool::new();
    assert!(pool.check_and_insert(&[0x01; 32]).is_ok());
    assert!(pool.check_and_insert(&[0x02; 32]).is_ok());
    assert!(pool.check_and_insert(&[0x03; 32]).is_ok());
}

#[cfg(feature = "blake3")]
#[test]
fn replay_salt_pool_rejects_replayed_salt() {
    use shadowsocks::ReplaySaltPool;
    let pool = ReplaySaltPool::new();
    let salt = vec![0xaau8; 32];
    assert!(pool.check_and_insert(&salt).is_ok());
    // Same salt within the window is a replay.
    assert!(pool.check_and_insert(&salt).is_err());
}

#[cfg(feature = "blake3")]
#[test]
fn replay_salt_pool_evicts_expired_entries() {
    use shadowsocks::ReplaySaltPool;
    use std::time::Duration;
    // A zero TTL evicts every entry on the next call, so a "replay" is never
    // observed — verifying the retention/eviction path is exercised.
    let pool = ReplaySaltPool::new_with_ttl(Duration::ZERO);
    let salt = vec![0xbbu8; 32];
    assert!(pool.check_and_insert(&salt).is_ok());
    assert!(
        pool.check_and_insert(&salt).is_ok(),
        "zero-TTL pool must evict the prior entry before the replay check"
    );
}
