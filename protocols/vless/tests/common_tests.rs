use ztls::common::*;

#[test]
fn test_strip_content_type_app_data() {
    let mut plaintext = vec![0x01, 0x02, 0x03, CONTENT_TYPE_APPLICATION_DATA];
    let ct = strip_content_type(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(plaintext, vec![0x01, 0x02, 0x03]);
}

#[test]
fn test_strip_content_type_handshake() {
    let mut plaintext = vec![0xAA, 0xBB, CONTENT_TYPE_HANDSHAKE];
    let ct = strip_content_type(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_HANDSHAKE);
    assert_eq!(plaintext, vec![0xAA, 0xBB]);
}

#[test]
fn test_strip_content_type_alert() {
    let mut plaintext = vec![0x01, 0x00, CONTENT_TYPE_ALERT];
    let ct = strip_content_type(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_ALERT);
    assert_eq!(plaintext, vec![0x01, 0x00]);
}

#[test]
fn test_strip_content_type_preserves_zeros() {
    // Trailing zeros in data should be preserved (not treated as padding)
    let mut plaintext = vec![0x01, 0x00, 0x00, CONTENT_TYPE_APPLICATION_DATA];
    let ct = strip_content_type(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(plaintext, vec![0x01, 0x00, 0x00]);
}

#[test]
fn test_strip_content_type_empty() {
    let mut plaintext = Vec::new();
    assert!(strip_content_type(&mut plaintext).is_err());
}

#[test]
fn test_strip_content_type_invalid() {
    let mut plaintext = vec![0x01, 0xFF]; // 0xFF is invalid
    assert!(strip_content_type(&mut plaintext).is_err());
}

#[test]
fn test_strip_with_padding_no_padding() {
    let mut plaintext = vec![0x01, 0x02, CONTENT_TYPE_APPLICATION_DATA];
    let ct = strip_content_type_with_padding(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(plaintext, vec![0x01, 0x02]);
}

#[test]
fn test_strip_with_padding_strips_zeros() {
    // TLS 1.3 format: content || type || padding
    let mut plaintext = vec![0x01, 0x02, CONTENT_TYPE_HANDSHAKE, 0x00, 0x00, 0x00];
    let ct = strip_content_type_with_padding(&mut plaintext).unwrap();
    assert_eq!(ct, CONTENT_TYPE_HANDSHAKE);
    assert_eq!(plaintext, vec![0x01, 0x02]);
}

#[test]
fn test_strip_with_padding_empty() {
    let mut plaintext = Vec::new();
    assert!(strip_content_type_with_padding(&mut plaintext).is_err());
}

#[test]
fn test_strip_with_padding_all_zeros() {
    let mut plaintext = vec![0x00, 0x00, 0x00];
    assert!(strip_content_type_with_padding(&mut plaintext).is_err());
}

#[test]
fn test_strip_with_padding_invalid_type() {
    let mut plaintext = vec![0x01, 0xFF, 0x00]; // 0xFF with padding
    assert!(strip_content_type_with_padding(&mut plaintext).is_err());
}
