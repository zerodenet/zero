use zero_protocol_vless::reality::common::*;
use zero_protocol_vless::reality::common::CONTENT_TYPE_HANDSHAKE;
use zero_protocol_vless::reality::reality_tls13_messages::*;

#[test]
fn test_construct_finished() {
    let verify_data = vec![0xCCu8; 32];
    let result = construct_finished(&verify_data);
    assert!(result.is_ok());
    let msg = result.unwrap();
    assert_eq!(msg[0], HANDSHAKE_TYPE_FINISHED);
    assert_eq!(msg.len(), 1 + 3 + 32);
}

#[test]
fn test_write_record_header() {
    let header = write_record_header(CONTENT_TYPE_HANDSHAKE, 100);
    assert_eq!(header.len(), 5);
    assert_eq!(header[0], 0x16);
    assert_eq!(header[1], 0x03);
    assert_eq!(header[2], 0x03);
    assert_eq!(u16::from_be_bytes([header[3], header[4]]), 100);
}
