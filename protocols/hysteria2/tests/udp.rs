//! Tests for Hysteria2 UDP datagram build/parse.
//!
//! Migrated from the inline `#[cfg(test)] mod tests` in
//! `protocols/hysteria2/src/udp.rs`.

use hysteria2::{
    build_udp_datagram, parse_udp_datagram, Hysteria2Outbound, Hysteria2UdpPacketTarget,
};
use zero_core::Address;
use zero_traits::UdpDatagramFraming;

#[test]
fn test_udp_datagram_roundtrip() {
    let addr = Address::Domain("example.com".into());
    let datagram = build_udp_datagram(1, 42, &addr, 443, b"hello").unwrap();
    let parsed = parse_udp_datagram(&datagram).unwrap();
    assert_eq!(parsed.session_id, 1);
    assert_eq!(parsed.packet_id, 42);
    assert_eq!(parsed.target, addr);
    assert_eq!(parsed.port, 443);
    assert_eq!(parsed.payload, b"hello");
}

#[test]
fn test_udp_datagram_ipv4() {
    let addr = Address::Ipv4([8, 8, 8, 8]);
    let datagram = build_udp_datagram(0, 0, &addr, 53, b"dns").unwrap();
    let parsed = parse_udp_datagram(&datagram).unwrap();
    assert_eq!(parsed.target, addr);
}

#[test]
fn udp_datagram_framing_trait_roundtrips_packet() {
    let target = Address::Domain("example.com".into());
    let datagram = <Hysteria2Outbound as UdpDatagramFraming<
        Hysteria2UdpPacketTarget<'_>,
        (),
    >>::encode_udp_datagram(
        &Hysteria2Outbound,
        &Hysteria2UdpPacketTarget {
            session_id: 7,
            packet_id: 9,
            target: &target,
            port: 8443,
            payload: b"h2",
        },
    )
    .expect("encode hysteria2 udp datagram");

    let decoded = <Hysteria2Outbound as UdpDatagramFraming<
        Hysteria2UdpPacketTarget<'_>,
        (),
    >>::decode_udp_datagram(&Hysteria2Outbound, &(), &datagram)
    .expect("decode hysteria2 udp datagram");

    assert_eq!(decoded.session_id, 7);
    assert_eq!(decoded.packet_id, 9);
    assert_eq!(decoded.target, target);
    assert_eq!(decoded.port, 8443);
    assert_eq!(decoded.payload, b"h2");
}
