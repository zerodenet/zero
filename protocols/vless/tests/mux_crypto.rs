use zero_protocol_vless::parse_uuid;
use zero_protocol_vless::MuxCrypto;

#[test]
fn test_key_derivation_determinism() {
    let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut crypto = MuxCrypto::new(&uuid);

    let plaintext = b"hello mux stream";
    let ct1 = crypto.encrypt_c2s(1, plaintext).unwrap();
    let mut crypto2 = MuxCrypto::new(&uuid);
    let pt = crypto2.decrypt_c2s(1, &ct1).unwrap();
    assert_eq!(pt, plaintext);
}

#[test]
fn test_roundtrip_both_directions() {
    let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut client = MuxCrypto::new(&uuid);
    let mut server = MuxCrypto::new(&uuid);

    let data = b"bidirectional test payload";
    let ct = client.encrypt_c2s(5, data).unwrap();
    let pt = server.decrypt_c2s(5, &ct).unwrap();
    assert_eq!(pt, data);

    let ct = server.encrypt_s2c(5, data).unwrap();
    let pt = client.decrypt_s2c(5, &ct).unwrap();
    assert_eq!(pt, data);
}

#[test]
fn test_multiple_streams_independent() {
    let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut crypto = MuxCrypto::new(&uuid);

    let ct1 = crypto.encrypt_c2s(1, b"stream 1 data").unwrap();
    let ct2 = crypto.encrypt_c2s(2, b"stream 2 data").unwrap();

    let mut crypto2 = MuxCrypto::new(&uuid);
    let pt1 = crypto2.decrypt_c2s(1, &ct1).unwrap();
    let pt2 = crypto2.decrypt_c2s(2, &ct2).unwrap();

    assert_eq!(pt1, b"stream 1 data");
    assert_eq!(pt2, b"stream 2 data");
}

#[test]
fn test_counter_increment() {
    let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut client = MuxCrypto::new(&uuid);

    let ct1 = client.encrypt_c2s(1, b"msg1").unwrap();
    let ct2 = client.encrypt_c2s(1, b"msg2").unwrap();
    assert_ne!(ct1, ct2);

    let mut server = MuxCrypto::new(&uuid);
    let pt1 = server.decrypt_c2s(1, &ct1).unwrap();
    let pt2 = server.decrypt_c2s(1, &ct2).unwrap();
    assert_eq!(pt1, b"msg1");
    assert_eq!(pt2, b"msg2");
}

#[test]
fn test_wrong_key_fails() {
    let uuid1 = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let uuid2 = parse_uuid("a831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut client = MuxCrypto::new(&uuid1);
    let mut server = MuxCrypto::new(&uuid2);

    let ct = client.encrypt_c2s(1, b"test").unwrap();
    assert!(server.decrypt_c2s(1, &ct).is_err());
}

#[test]
fn test_empty_payload() {
    let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
    let mut client = MuxCrypto::new(&uuid);
    let mut server = MuxCrypto::new(&uuid);

    let ct = client.encrypt_c2s(1, b"").unwrap();
    assert_eq!(ct.len(), 16);
    let pt = server.decrypt_c2s(1, &ct).unwrap();
    assert!(pt.is_empty());
}
