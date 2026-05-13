use crate::reality::reality_util::*;

#[test]
fn test_decode_short_id() {
    let short_id = decode_short_id("0123456789abcdef").unwrap();
    assert_eq!(short_id, [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    let short_id2 = decode_short_id("abcdef").unwrap();
    assert_eq!(short_id2, [0xab, 0xcd, 0xef, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let short_id3 = decode_short_id("").unwrap();
    assert_eq!(short_id3, [0; 8]);
    assert!(decode_short_id("0123456789abcdef0").is_err());
}

#[test]
fn test_extract_client_random() {
    let mut client_hello = vec![0u8; 100];
    client_hello[0] = 0x16;
    client_hello[1] = 0x03;
    client_hello[2] = 0x03;
    client_hello[5] = 0x01;
    client_hello[9] = 0x03;
    client_hello[10] = 0x03;
    for i in 0..32 { client_hello[11 + i] = (i + 1) as u8; }
    let random = extract_client_random(&client_hello).unwrap();
    for (index, byte) in random.iter().enumerate() {
        assert_eq!(*byte, (index + 1) as u8);
    }
}

#[test]
fn test_decode_public_key() {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let key_bytes = [0x42u8; 32];
    let encoded = URL_SAFE_NO_PAD.encode(key_bytes);
    let decoded = decode_public_key(&encoded).unwrap();
    assert_eq!(decoded, key_bytes);
    let short_key = [0x42u8; 16];
    let encoded_short = URL_SAFE_NO_PAD.encode(short_key);
    assert!(decode_public_key(&encoded_short).is_err());
    assert!(decode_public_key("not-valid-base64!!!").is_err());
}
