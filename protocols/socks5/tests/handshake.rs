use std::collections::VecDeque;

use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_protocol_socks5::{
    build_udp_packet, parse_udp_packet, Socks5Inbound, Socks5Outbound, Socks5Request,
};
use zero_traits::AsyncSocket;

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

#[tokio::test]
async fn parses_connect_request_with_domain_target() {
    let mut socket = MockSocket::new(&[
        0x05, 0x01, 0x00, // method negotiation
        0x05, 0x01, 0x00, 0x03, // connect + domain
        0x0b, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o', b'm', 0x01,
        0xbb, // port 443
    ]);

    let session = Socks5Inbound
        .handshake(&mut socket)
        .await
        .expect("handshake");

    assert_eq!(session.target, Address::Domain("example.com".into()));
    assert_eq!(session.port, 443);
    assert_eq!(session.network, Network::Tcp);
    assert_eq!(session.protocol, ProtocolType::Socks5);
    assert_eq!(
        socket.writes,
        vec![
            0x05, 0x00, // auth accepted
            0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0 // connect success
        ]
    );
}

#[tokio::test]
async fn parses_connect_request_with_ipv4_target() {
    let mut socket = MockSocket::new(&[
        0x05, 0x01, 0x00, // method negotiation
        0x05, 0x01, 0x00, 0x01, // connect + ipv4
        1, 2, 3, 4, 0x00, 0x50, // port 80
    ]);

    let session = Socks5Inbound
        .handshake(&mut socket)
        .await
        .expect("handshake");

    assert_eq!(session.target, Address::Ipv4([1, 2, 3, 4]));
    assert_eq!(session.port, 80);
}

#[tokio::test]
async fn rejects_unsupported_auth_method() {
    let mut socket = MockSocket::new(&[0x05, 0x01, 0x02]);

    let error = Socks5Inbound
        .handshake(&mut socket)
        .await
        .expect_err("should fail");

    assert_eq!(
        error,
        Error::Unsupported("SOCKS5 auth method is not supported")
    );
    assert_eq!(socket.writes, vec![0x05, 0xff]);
}

#[tokio::test]
async fn rejects_unsupported_command() {
    let mut socket = MockSocket::new(&[
        0x05, 0x01, 0x00, // method negotiation
        0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
        127, 0, 0, 1, 0x00, 0x35,
    ]);

    let error = Socks5Inbound
        .handshake(&mut socket)
        .await
        .expect_err("should fail");

    assert_eq!(error, Error::Unsupported("SOCKS5 command is not supported"));
    assert_eq!(
        socket.writes,
        vec![
            0x05, 0x00, // auth accepted
            0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[tokio::test]
async fn parses_udp_associate_request() {
    let mut socket = MockSocket::new(&[
        0x05, 0x01, 0x00, // method negotiation
        0x05, 0x03, 0x00, 0x01, // udp associate + ipv4
        0, 0, 0, 0, 0x00, 0x00,
    ]);

    let request = Socks5Inbound
        .accept_command(&mut socket)
        .await
        .expect("request");

    match request {
        Socks5Request::UdpAssociate(request) => {
            assert_eq!(request.client_hint, Address::Ipv4([0, 0, 0, 0]));
            assert_eq!(request.client_port, 0);
        }
        Socks5Request::Connect(_) => panic!("expected udp associate"),
    }

    assert_eq!(socket.writes, vec![0x05, 0x00]);
}

#[tokio::test]
async fn outbound_establishes_tunnel_for_domain_target() {
    let mut socket = MockSocket::new(&[
        0x05, 0x00, // auth accepted
        0x05, 0x00, 0x00, 0x01, // connect success + ipv4
        127, 0, 0, 1, 0x00, 0x50,
    ]);
    let session = Session::new(
        0,
        Address::Domain("example.com".into()),
        443,
        Network::Tcp,
        ProtocolType::Socks5,
    );

    Socks5Outbound
        .establish_tunnel(&mut socket, &session)
        .await
        .expect("tunnel");

    assert_eq!(
        socket.writes,
        vec![
            0x05, 0x01, 0x00, // auth negotiation
            0x05, 0x01, 0x00, 0x03, 0x0b, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c',
            b'o', b'm', 0x01, 0xbb
        ]
    );
}

#[tokio::test]
async fn outbound_rejects_upstream_failure_reply() {
    let mut socket = MockSocket::new(&[
        0x05, 0x00, // auth accepted
        0x05, 0x04, 0x00, 0x01, // host unreachable
        127, 0, 0, 1, 0x00, 0x50,
    ]);
    let session = Session::new(
        0,
        Address::Ipv4([1, 1, 1, 1]),
        53,
        Network::Tcp,
        ProtocolType::Socks5,
    );

    let error = Socks5Outbound
        .establish_tunnel(&mut socket, &session)
        .await
        .expect_err("fail");

    assert_eq!(error, Error::Route("SOCKS5 upstream host unreachable"));
}

#[tokio::test]
async fn outbound_establishes_udp_association() {
    let mut socket = MockSocket::new(&[
        0x05, 0x00, // auth accepted
        0x05, 0x00, 0x00, 0x01, // udp associate success + ipv4
        127, 0, 0, 1, 0x20, 0x00,
    ]);

    let (address, port) = Socks5Outbound
        .establish_udp_association(&mut socket)
        .await
        .expect("udp association");

    assert_eq!(address, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(port, 0x2000);
    assert_eq!(
        socket.writes,
        vec![
            0x05, 0x01, 0x00, // auth negotiation
            0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0 // udp associate
        ]
    );
}

#[test]
fn builds_and_parses_udp_packet() {
    let packet = build_udp_packet(&Address::Domain("example.com".into()), 5353, b"ping")
        .expect("build packet");
    let parsed = parse_udp_packet(&packet).expect("parse packet");

    assert_eq!(parsed.target, Address::Domain("example.com".into()));
    assert_eq!(parsed.port, 5353);
    assert_eq!(parsed.payload, b"ping");
}
