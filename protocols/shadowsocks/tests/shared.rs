//! Tests for Shadowsocks protocol constants and helpers.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/shadowsocks/src/shared.rs`.
//!
//! The AEAD/TCP-chunk round-trips require the `crypto` feature; the
//! cipher/address/target tests run unconditionally.

use shadowsocks::{
    build_target_data, decode_address, encode_address, parse_target_data, CipherKind,
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
