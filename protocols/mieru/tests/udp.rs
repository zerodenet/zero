//! Tests for mieru UDP associate encapsulation.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/mieru/src/udp.rs`.

use mieru::{unwrap_udp_associate, wrap_udp_associate, MieruProtocol, MieruUdpAssociatePacket};
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
