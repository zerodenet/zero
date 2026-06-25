//! Tests for mieru UDP associate encapsulation.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/mieru/src/udp.rs`.

use mieru::{
    decode_inbound_udp_packet, encode_udp_response, unwrap_udp_associate, wrap_udp_associate,
    MieruProtocol, MieruUdpAssociatePacket,
};
use zero_core::Address;
use zero_traits::UdpPacketFraming;

#[test]
fn test_udp_associate_roundtrip() {
    let original = b"hello udp";
    let wrapped = wrap_udp_associate(original);
    let unwrapped = unwrap_udp_associate(&wrapped).unwrap();
    assert_eq!(&unwrapped, original);
}

#[test]
fn test_unwrap_invalid() {
    assert!(unwrap_udp_associate(&[]).is_err());
    assert!(unwrap_udp_associate(&[0x01, 0x00, 0x01, 0x00, 0xff]).is_err());
    assert!(unwrap_udp_associate(&[0x00, 0x00, 0x05, 0x00]).is_err());
}

#[test]
fn udp_packet_framing_trait_roundtrips_associate_payload() {
    let encoded =
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::encode_udp_packet(
            &MieruProtocol,
            &MieruUdpAssociatePacket {
                payload: b"mieru udp",
            },
        )
        .expect("encode mieru udp associate payload");
    let decoded =
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &encoded,
        )
        .expect("decode mieru udp associate payload");

    assert_eq!(decoded.payload, b"mieru udp");
}

#[test]
fn inbound_udp_packet_decoder_unwraps_socks5_payload() {
    let frame = encode_udp_response(&Address::Domain("dns.example".to_owned()), 5353, b"query")
        .expect("encode response frame");

    let decoded = decode_inbound_udp_packet(&frame).expect("decode inbound packet");

    assert_eq!(decoded.target, Address::Domain("dns.example".to_owned()));
    assert_eq!(decoded.port, 5353);
    assert_eq!(decoded.payload, b"query");
}

#[test]
fn udp_response_encoder_wraps_socks5_payload() {
    let frame = encode_udp_response(&Address::Ipv4([1, 1, 1, 1]), 53, b"answer")
        .expect("encode response frame");
    let unwrapped = unwrap_udp_associate(&frame).expect("unwrap response frame");

    assert_eq!(&unwrapped[..4], &[0, 0, 0, 0x01]);
    assert_eq!(&unwrapped[4..8], &[1, 1, 1, 1]);
    assert_eq!(u16::from_be_bytes([unwrapped[8], unwrapped[9]]), 53);
    assert_eq!(&unwrapped[10..], b"answer");
}
