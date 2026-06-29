use std::collections::VecDeque;

use vless::udp::VlessUdpPacketV2Codec;
use vless::{format_uuid, parse_uuid, VlessInbound, VlessOutbound, VlessUser, VlessUserStore};
use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;
#[cfg(feature = "reality")]
use zero_traits::DeferredTcpTunnelProtocol;
use zero_traits::{UdpPacketFraming, UdpPacketTunnelProtocol};

const USER_ID: &str = "11111111-2222-3333-4444-555555555555";

#[derive(Debug, Default)]
struct MockSocket {
    reads: VecDeque<u8>,
    writes: Vec<u8>,
    shutdown_called: bool,
}

impl MockSocket {
    fn new(input: &[u8]) -> Self {
        Self {
            reads: input.iter().copied().collect(),
            writes: Vec::new(),
            shutdown_called: false,
        }
    }
}

impl AsyncSocket for MockSocket {
    type Error = ();

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut read = 0;

        while read < buf.len() {
            let Some(byte) = self.reads.pop_front() else {
                break;
            };
            buf[read] = byte;
            read += 1;
        }

        Ok(read)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.writes.extend_from_slice(buf);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.shutdown_called = true;
        Ok(())
    }
}

struct TestUsers {
    id: [u8; 16],
}

impl VlessUserStore for TestUsers {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        if id == &self.id {
            Some(VlessUser {
                credential_id: Some("node-user-1".to_owned()),
                principal_key: Some("user:10001".to_owned()),
                flow: None,
                up_bps: None,
                down_bps: None,
            })
        } else {
            None
        }
    }
}

#[test]
fn parses_and_formats_uuid() {
    let id = parse_uuid(USER_ID).expect("uuid");

    assert_eq!(format_uuid(&id), USER_ID);
    assert_eq!(parse_uuid("11111111222233334444555555555555"), Ok(id));
    assert!(parse_uuid("not-a-uuid").is_err());
}

#[tokio::test]
async fn inbound_accepts_authorized_tcp_request_with_domain_target() {
    let id = parse_uuid(USER_ID).expect("uuid");
    let mut request = vec![0x00];
    request.extend_from_slice(&id);
    request.extend_from_slice(&[
        0x00, // addon length
        0x01, // tcp command
        0x01, 0xbb, // port 443
        0x02, // domain
        0x0b,
    ]);
    request.extend_from_slice(b"example.com");

    let mut socket = MockSocket::new(&request);
    let users = TestUsers { id };

    let session = VlessInbound
        .handshake_with_auth(&mut socket, &users)
        .await
        .expect("handshake");

    assert_eq!(session.target, Address::Domain("example.com".into()));
    assert_eq!(session.port, 443);
    assert_eq!(session.network, Network::Tcp);
    assert_eq!(session.protocol, ProtocolType::Vless);
    let auth = session.auth.expect("auth");
    assert_eq!(auth.scheme, "vless");
    assert_eq!(auth.credential_id.as_deref(), Some("node-user-1"));
    assert_eq!(auth.principal_key.as_deref(), Some("user:10001"));
    assert_eq!(socket.writes, vec![0x00, 0x00]);
}

#[tokio::test]
async fn inbound_rejects_unknown_user() {
    let mut request = vec![0x00];
    request.extend_from_slice(&[9_u8; 16]);
    request.extend_from_slice(&[
        0x00, // addon length
        0x01, // tcp command
        0x00, 0x50, // port 80
        0x01, // ipv4
        127, 0, 0, 1,
    ]);
    let users = TestUsers {
        id: parse_uuid(USER_ID).expect("uuid"),
    };
    let mut socket = MockSocket::new(&request);

    let error = VlessInbound
        .accept_tcp_with_auth(&mut socket, &users)
        .await
        .expect_err("should fail");

    assert_eq!(error, Error::Unsupported("VLESS user is not authorized"));
    assert!(socket.writes.is_empty());
}

#[tokio::test]
async fn outbound_establishes_tcp_tunnel_for_ipv4_target() {
    let id = parse_uuid(USER_ID).expect("uuid");
    let mut socket = MockSocket::new(&[
        0x00, 0x00, // response version + addon length
    ]);
    let session = Session::new(
        0,
        Address::Ipv4([127, 0, 0, 1]),
        8080,
        Network::Tcp,
        ProtocolType::Vless,
    );

    VlessOutbound
        .establish_tcp_tunnel(&mut socket, &session, &id)
        .await
        .expect("tunnel");

    let mut expected = vec![0x00];
    expected.extend_from_slice(&id);
    expected.extend_from_slice(&[
        0x00, // addon length
        0x01, // tcp command
        0x1f, 0x90, // port 8080
        0x01, // ipv4
        127, 0, 0, 1,
    ]);
    assert_eq!(socket.writes, expected);
}

#[cfg(feature = "reality")]
#[tokio::test]
async fn outbound_deferred_tcp_tunnel_request_does_not_read_response() {
    let id = parse_uuid(USER_ID).expect("uuid");
    let mut socket = MockSocket::new(&[]);
    let session = Session::new(
        0,
        Address::Ipv4([127, 0, 0, 1]),
        8080,
        Network::Tcp,
        ProtocolType::Vless,
    );

    VlessOutbound
        .send_deferred_tcp_tunnel_request(
            &mut socket,
            &vless::VlessFlowTcpTunnelTarget {
                session: &session,
                id: &id,
                flow: None,
            },
        )
        .await
        .expect("deferred tunnel request");

    let mut expected = vec![0x00];
    expected.extend_from_slice(&id);
    expected.extend_from_slice(&[
        0x00, // addon length
        0x01, // tcp command
        0x1f, 0x90, // port 8080
        0x01, // ipv4
        127, 0, 0, 1,
    ]);
    assert_eq!(socket.writes, expected);
}

#[tokio::test]
async fn outbound_establishes_udp_packet_tunnel_and_consumes_response() {
    let id = parse_uuid(USER_ID).expect("uuid");
    let mut socket = MockSocket::new(&[
        0x00, 0x00, // response version + addon length
    ]);
    let session = Session::new(
        0,
        Address::Ipv4([127, 0, 0, 1]),
        5353,
        Network::Udp,
        ProtocolType::Vless,
    );

    <VlessOutbound as UdpPacketTunnelProtocol<vless::udp::VlessUdpPacketTunnelTarget>>::establish_udp_packet_tunnel(
        &VlessOutbound,
        &mut socket,
        &vless::udp::VlessUdpPacketTunnelTarget {
            session: &session,
            id: &id,
        },
    )
    .await
    .expect("udp packet tunnel");

    let mut expected = vec![0x00];
    expected.extend_from_slice(&id);
    expected.extend_from_slice(&[
        0x00, // addon length
        0x02, // udp command
        0x14, 0xe9, // port 5353
        0x01, // ipv4
        127, 0, 0, 1,
    ]);
    assert_eq!(socket.writes, expected);
    assert!(socket.reads.is_empty());
}

#[tokio::test]
async fn inbound_accepts_authorized_udp_request_with_ipv4_target() {
    let id = parse_uuid(USER_ID).expect("uuid");
    let mut request = vec![0x00];
    request.extend_from_slice(&id);
    request.extend_from_slice(&[
        0x00, // addon length
        0x02, // udp command
        0x00, 0x35, // port 53
        0x01, // ipv4
        8, 8, 8, 8,
    ]);

    let mut socket = MockSocket::new(&request);
    let users = TestUsers { id };

    let session = VlessInbound
        .handshake_with_auth(&mut socket, &users)
        .await
        .expect("handshake");

    assert_eq!(session.target, Address::Ipv4([8, 8, 8, 8]));
    assert_eq!(session.port, 53);
    assert_eq!(session.network, Network::Udp);
    assert_eq!(session.protocol, ProtocolType::Vless);
}

#[test]
fn parse_udp_packet_with_ipv4() {
    let packet =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::encode_udp_packet(
            &VlessOutbound,
            &vless::udp::VlessUdpPacketTarget {
                address: &Address::Ipv4([8, 8, 8, 8]),
                port: 53,
                payload: b"dns query",
            },
        )
        .expect("build");
    let parsed =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::decode_udp_packet(
            &VlessOutbound,
            &packet,
        )
        .expect("parse");
    assert_eq!(parsed.target(), &Address::Ipv4([8, 8, 8, 8]));
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), b"dns query");
}

#[test]
fn parse_udp_packet_with_domain() {
    let packet =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::encode_udp_packet(
            &VlessOutbound,
            &vless::udp::VlessUdpPacketTarget {
                address: &Address::Domain("example.com".into()),
                port: 443,
                payload: b"udp payload",
            },
        )
        .expect("build");
    let parsed =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::decode_udp_packet(
            &VlessOutbound,
            &packet,
        )
        .expect("parse");
    assert_eq!(parsed.target(), &Address::Domain("example.com".into()));
    assert_eq!(parsed.port(), 443);
    assert_eq!(parsed.payload(), b"udp payload");
}

#[test]
fn build_udp_packet_with_ipv6() {
    let payload = b"hello v6";
    let address = Address::Ipv6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    let packet =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::encode_udp_packet(
            &VlessOutbound,
            &vless::udp::VlessUdpPacketTarget {
                address: &address,
                port: 53,
                payload,
            },
        )
        .expect("build");

    let parsed =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::decode_udp_packet(
            &VlessOutbound,
            &packet,
        )
        .expect("parse");
    assert_eq!(
        parsed.target(),
        &Address::Ipv6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    );
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), payload);
}

// ── v2 auto-detect tests ──

#[test]
fn parse_udp_v2_with_address() {
    // [marker:2][flags:1(0x01)][port:2][atyp ipv4:1][addr:4][payload]
    let mut packet = vec![
        0x00, 0x00, // v2 marker
        0x01, // flags: has address
        0x00, 0x35, // port 53
        0x01, // ipv4
        8, 8, 8, 8,
    ];
    packet.extend_from_slice(b"dns query v2");

    let parsed = VlessUdpPacketV2Codec
        .decode_packet(&packet, None, None)
        .expect("parse v2");
    assert_eq!(parsed.target(), &Address::Ipv4([8, 8, 8, 8]));
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), b"dns query v2");
}

#[test]
fn parse_udp_v2_without_address_reuse_cache() {
    // [marker:2][flags:1(0x00)][payload]
    let mut packet = vec![
        0x00, 0x00, // v2 marker
        0x00, // flags: no address
    ];
    packet.extend_from_slice(b"reuse address");

    let cached = Address::Domain("example.com".into());
    let parsed = VlessUdpPacketV2Codec
        .decode_packet(&packet, Some(&cached), Some(443))
        .expect("parse v2 reuse");
    assert_eq!(parsed.target(), &cached);
    assert_eq!(parsed.port(), 443);
    assert_eq!(parsed.payload(), b"reuse address");
}

#[test]
fn parse_udp_v2_without_address_fails_without_cache() {
    let packet = vec![0x00, 0x00, 0x00, b'x'];
    let err = VlessUdpPacketV2Codec
        .decode_packet(&packet, None, None)
        .unwrap_err();
    assert!(
        err.to_string().contains("cached"),
        "expected cache error, got: {err}"
    );
}

#[test]
fn parse_udp_v2_falls_back_to_v1() {
    // v1 format: [port:2][atyp:1][addr:4][payload]
    let mut packet = vec![0x00, 0x35, 0x01, 8, 8, 8, 8];
    packet.extend_from_slice(b"v1 fallback");

    let parsed = VlessUdpPacketV2Codec
        .decode_packet(&packet, None, None)
        .expect("v1 fallback");
    assert_eq!(parsed.target(), &Address::Ipv4([8, 8, 8, 8]));
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), b"v1 fallback");
}

#[test]
fn build_udp_v2_with_address() {
    let packet = VlessUdpPacketV2Codec
        .encode_packet(
            &Address::Ipv4([1, 1, 1, 1]),
            8080,
            b"hello",
            false, // include address
        )
        .expect("build v2");
    assert_eq!(&packet[..2], &[0x00, 0x00]); // marker
    assert_eq!(packet[2], 0x01); // flags: has address
    assert_eq!(u16::from_be_bytes([packet[3], packet[4]]), 8080);
    assert_eq!(packet[5], 0x01); // ipv4

    let parsed = VlessUdpPacketV2Codec
        .decode_packet(&packet, None, None)
        .expect("roundtrip");
    assert_eq!(parsed.target(), &Address::Ipv4([1, 1, 1, 1]));
    assert_eq!(parsed.port(), 8080);
    assert_eq!(parsed.payload(), b"hello");
}

#[test]
fn build_udp_v2_omit_address() {
    let packet = VlessUdpPacketV2Codec
        .encode_packet(
            &Address::Ipv4([0, 0, 0, 0]), // unused when omitting
            0,                            // unused when omitting
            b"streaming",
            true, // omit address
        )
        .expect("build v2 omit");
    assert_eq!(&packet[..2], &[0x00, 0x00]); // marker
    assert_eq!(packet[2], 0x00); // flags: no address
    assert_eq!(&packet[3..], b"streaming"); // payload starts after flags
}

#[test]
fn inbound_udp_decoder_parses_client_packet() {
    let packet =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::encode_udp_packet(
            &VlessOutbound,
            &vless::udp::VlessUdpPacketTarget {
                address: &Address::Domain("dns.example".into()),
                port: 5353,
                payload: b"query",
            },
        )
        .expect("build packet");

    let parsed = vless::udp::VlessInboundUdpSession::new()
        .decode_request(&packet)
        .expect("decode inbound packet");

    assert_eq!(parsed.target(), &Address::Domain("dns.example".into()));
    assert_eq!(parsed.port(), 5353);
    assert_eq!(parsed.payload(), b"query");
}

#[test]
fn udp_response_encoder_builds_response_packet() {
    let packet = vless::udp::VlessInboundUdpCodec
        .encode_response(&Address::Ipv4([1, 1, 1, 1]), 53, b"answer")
        .expect("encode response");

    let parsed =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::decode_udp_packet(
            &VlessOutbound,
            &packet,
        )
        .expect("parse response packet");
    assert_eq!(parsed.target(), &Address::Ipv4([1, 1, 1, 1]));
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), b"answer");
}

#[test]
fn mux_udp_response_encoder_wraps_vless_packet() {
    let (down_tx, mut down_rx) = tokio::sync::mpsc::unbounded_channel::<(u16, Vec<u8>)>();
    let writer = vless::VlessInboundMuxWriter::new(down_tx);
    vless::udp::VlessInboundUdpCodec
        .send_mux_response(&writer, 7, &Address::Ipv4([8, 8, 8, 8]), 53, b"dns")
        .expect("encode mux response");
    let (sid, frame) = down_rx.try_recv().expect("mux response frame");
    assert_eq!(sid, 7);

    assert_eq!(u16::from_be_bytes([frame[0], frame[1]]), 4 + 7 + 3);
    assert_eq!(u16::from_be_bytes([frame[2], frame[3]]), 7);
    assert_eq!(frame[4], vless::STATUS_KEEP);
    assert_eq!(frame[5], vless::OPTION_DATA);

    let parsed =
        <VlessOutbound as UdpPacketFraming<vless::udp::VlessUdpPacketTarget>>::decode_udp_packet(
            &VlessOutbound,
            &frame[6..],
        )
        .expect("parse mux payload");
    assert_eq!(parsed.target(), &Address::Ipv4([8, 8, 8, 8]));
    assert_eq!(parsed.port(), 53);
    assert_eq!(parsed.payload(), b"dns");
}
