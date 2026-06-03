#![cfg(feature = "crypto")]

use std::io;

use zero_core::{Address, Network, ProtocolType, Session};
use zero_protocol_shadowsocks::{
    decrypt_tcp_chunk_length, decrypt_tcp_chunk_payload, derive_key, parse_target_data, CipherKind,
    ShadowsocksOutbound, TCP_CHUNK_SIZE_LEN,
};
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
async fn outbound_writes_salt_and_first_chunk_in_one_write() {
    let cipher = CipherKind::Aes128Gcm;
    let password = b"test-password";
    let session = Session::new(
        0,
        Address::Domain("www.gstatic.com".to_owned()),
        80,
        Network::Tcp,
        ProtocolType::Shadowsocks,
    );
    let mut socket = RecordingSocket::default();

    let outbound_session = ShadowsocksOutbound
        .send_request(&mut socket, &session, cipher, password)
        .await
        .expect("send shadowsocks request");

    assert_eq!(socket.writes.len(), 1);
    assert_eq!(outbound_session.next_upload_nonce, 2);

    let request = &socket.writes[0];
    let salt_len = cipher.salt_len();
    let length_size = TCP_CHUNK_SIZE_LEN + cipher.tag_len();
    assert!(request.len() > salt_len + length_size);

    let key = derive_key(password, &request[..salt_len], cipher.key_len()).unwrap();
    let mut nonce = 0;
    let payload_len = decrypt_tcp_chunk_length(
        cipher,
        &key,
        &mut nonce,
        &request[salt_len..salt_len + length_size],
    )
    .unwrap();
    let plain = decrypt_tcp_chunk_payload(
        cipher,
        &key,
        &mut nonce,
        payload_len,
        &request[salt_len + length_size..],
    )
    .unwrap();

    let (target, port, payload_offset) = parse_target_data(&plain).unwrap();
    assert_eq!(target, Address::Domain("www.gstatic.com".to_owned()));
    assert_eq!(port, 80);
    assert_eq!(&plain[payload_offset..], b"");
}
