use std::collections::VecDeque;

use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_protocol_vless::{
    format_uuid, parse_uuid, VlessInbound, VlessOutbound, VlessUser, VlessUserStore,
};
use zero_traits::AsyncSocket;

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
