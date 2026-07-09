use std::collections::VecDeque;

use http_connect::{HttpConnectInbound, HttpConnectResponse};
use zero_core::Address;
use zero_traits::AsyncSocket;

#[derive(Debug, Default)]
struct MockSocket {
    reads: VecDeque<u8>,
    writes: Vec<u8>,
}

impl MockSocket {
    fn new(input: &[u8]) -> Self {
        Self {
            reads: input.iter().copied().collect(),
            writes: Vec::new(),
        }
    }
}

impl AsyncSocket for MockSocket {
    type Error = ();

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move {
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
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            self.writes.extend_from_slice(buf);
            Ok(())
        }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { Ok(()) }
    }
}

#[tokio::test]
async fn parses_domain_authority() {
    let request = b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n";
    let mut socket = MockSocket::new(request);

    let session = HttpConnectInbound
        .accept_request(&mut socket)
        .await
        .expect("request");

    assert_eq!(session.target, Address::Domain("example.com".to_string()));
    assert_eq!(session.port, 443);
}

#[tokio::test]
async fn parses_ipv4_authority() {
    let request = b"CONNECT 127.0.0.1:8080 HTTP/1.1\r\nHost: 127.0.0.1:8080\r\n\r\n";
    let mut socket = MockSocket::new(request);

    let session = HttpConnectInbound
        .accept_request(&mut socket)
        .await
        .expect("request");

    assert_eq!(session.target, Address::Ipv4([127, 0, 0, 1]));
    assert_eq!(session.port, 8080);
}

#[tokio::test]
async fn rejects_non_connect_method() {
    let request = b"GET example.com:443 HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let mut socket = MockSocket::new(request);

    let error = HttpConnectInbound
        .accept_request(&mut socket)
        .await
        .expect_err("error");

    assert_eq!(
        error,
        zero_core::Error::Unsupported("HTTP method is not supported")
    );
}

#[tokio::test]
async fn writes_connection_established_response() {
    let mut socket = MockSocket::default();

    HttpConnectInbound
        .send_response(&mut socket, HttpConnectResponse::ConnectionEstablished)
        .await
        .expect("response");

    assert_eq!(
        socket.writes,
        b"HTTP/1.1 200 Connection Established\r\n\r\n"
    );
}
