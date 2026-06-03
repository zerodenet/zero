#![cfg(feature = "crypto")]

use std::io;

use zero_core::{Address, Network, ProtocolType, Session};
use zero_protocol_trojan::{TrojanOutbound, CMD_TCP, CRLF, PASSWORD_HASH_LEN};
use zero_traits::AsyncSocket;

#[derive(Default)]
struct RecordingSocket {
    writes: Vec<Vec<u8>>,
}

impl AsyncSocket for RecordingSocket {
    type Error = io::Error;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(0)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.writes.push(buf.to_vec());
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn outbound_writes_complete_request_in_one_write() {
    let session = Session::new(
        0,
        Address::Domain("www.gstatic.com".to_owned()),
        80,
        Network::Tcp,
        ProtocolType::Trojan,
    );
    let mut socket = RecordingSocket::default();

    TrojanOutbound
        .send_request(&mut socket, &session, "test-password")
        .await
        .expect("send trojan request");

    assert_eq!(socket.writes.len(), 1);
    let request = &socket.writes[0];
    assert_eq!(&request[PASSWORD_HASH_LEN..PASSWORD_HASH_LEN + 2], CRLF);
    assert_eq!(request[PASSWORD_HASH_LEN + 2], CMD_TCP);
    assert_eq!(
        request[PASSWORD_HASH_LEN + 3],
        zero_protocol_trojan::ATYP_DOMAIN
    );
    assert_eq!(
        request[PASSWORD_HASH_LEN + 4] as usize,
        "www.gstatic.com".len()
    );
    assert_eq!(
        &request[PASSWORD_HASH_LEN + 5..PASSWORD_HASH_LEN + 20],
        b"www.gstatic.com"
    );
    assert_eq!(
        u16::from_be_bytes([
            request[PASSWORD_HASH_LEN + 20],
            request[PASSWORD_HASH_LEN + 21]
        ]),
        80
    );
    assert_eq!(
        &request[PASSWORD_HASH_LEN + 22..PASSWORD_HASH_LEN + 24],
        CRLF
    );
}
