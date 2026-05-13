use zero_core::Address;
use zero_protocol_vless::{
    flow_build_request, flow_byte, flow_from_byte, parse_flow, parse_uuid, FLOW_XTLS_RPRX_VISION,
    FLOW_XTLS_RPRX_VISION_UDP,
};

#[test]
fn test_flow_roundtrip() {
    let uuid_str = "b831381d-6324-4d53-ad4f-8cda48b30811";
    let uuid = parse_uuid(uuid_str).unwrap();
    let flow = Some(FLOW_XTLS_RPRX_VISION);

    let address = Address::Domain("example.com".into());
    let (fbyte, payload) = flow_build_request(&uuid, flow, 0x01, 443, &address).unwrap();

    assert_eq!(fbyte, 0x01);
    assert!(payload.len() >= 8 + 1 + 2 + 16);
}

#[test]
fn test_plain_no_flow() {
    let uuid_str = "b831381d-6324-4d53-ad4f-8cda48b30811";
    let uuid = parse_uuid(uuid_str).unwrap();

    let address = Address::Ipv4([127, 0, 0, 1]);
    let (fbyte, payload) = flow_build_request(&uuid, None, 0x01, 80, &address).unwrap();

    assert_eq!(fbyte, 0x00);
    assert_eq!(payload.len(), 8);
    assert_eq!(payload[0], 0x01);
    assert_eq!(u16::from_be_bytes([payload[1], payload[2]]), 80);
}

#[test]
fn test_parse_flow_valid() {
    assert!(parse_flow(FLOW_XTLS_RPRX_VISION).is_ok());
    assert!(parse_flow(FLOW_XTLS_RPRX_VISION_UDP).is_ok());
}

#[test]
fn test_parse_flow_invalid() {
    assert!(parse_flow("unknown-flow").is_err());
    assert!(parse_flow("").is_err());
}

#[test]
fn test_flow_byte_mapping() {
    assert_eq!(flow_byte(Some(FLOW_XTLS_RPRX_VISION)), 0x01);
    assert_eq!(flow_byte(None), 0x00);
    assert_eq!(flow_from_byte(0x01), Some(FLOW_XTLS_RPRX_VISION));
    assert_eq!(flow_from_byte(0x00), None);
    assert_eq!(flow_from_byte(0xFF), None);
}
