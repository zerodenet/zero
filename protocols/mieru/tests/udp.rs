//! Tests for mieru UDP associate encapsulation.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/mieru/src/udp.rs`.

use mieru::{MieruProtocol, MieruUdpAssociatePacket, MieruUdpFlowCodec};
use zero_core::Address;
use zero_traits::{DatagramCodec, UdpPacketFraming};

#[test]
fn test_udp_associate_roundtrip() {
    let original = b"hello udp";
    let wrapped =
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::encode_udp_packet(
            &MieruProtocol,
            &MieruUdpAssociatePacket { payload: original },
        )
        .unwrap();
    let unwrapped =
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &wrapped,
        )
        .unwrap();
    assert_eq!(&unwrapped.payload, original);
}

#[test]
fn test_unwrap_invalid() {
    assert!(
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &[],
        )
        .is_err()
    );
    assert!(
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &[0x01, 0x00, 0x01, 0x00, 0xff],
        )
        .is_err()
    );
    assert!(
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &[0x00, 0x00, 0x05, 0x00],
        )
        .is_err()
    );
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
    let codec = MieruUdpFlowCodec;
    let frame = codec
        .encode(&Address::Domain("dns.example".to_owned()), 5353, b"query")
        .expect("encode response frame");

    let decoded = codec.decode(&frame).expect("decode inbound packet");

    assert_eq!(decoded.0, Address::Domain("dns.example".to_owned()));
    assert_eq!(decoded.1, 5353);
    assert_eq!(decoded.2, b"query");
}

#[test]
fn udp_response_encoder_wraps_socks5_payload() {
    let frame = MieruUdpFlowCodec
        .encode(&Address::Ipv4([1, 1, 1, 1]), 53, b"answer")
        .expect("encode response frame");
    let unwrapped =
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            &frame,
        )
        .expect("unwrap response frame");

    assert_eq!(&unwrapped.payload[..4], &[0, 0, 0, 0x01]);
    assert_eq!(&unwrapped.payload[4..8], &[1, 1, 1, 1]);
    assert_eq!(
        u16::from_be_bytes([unwrapped.payload[8], unwrapped.payload[9]]),
        53
    );
    assert_eq!(&unwrapped.payload[10..], b"answer");
}
