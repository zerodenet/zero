#![cfg(feature = "reality")]

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vless::{DeferredVlessResponseStream, VLESS_VERSION};

#[tokio::test]
async fn deferred_vless_response_discards_header_before_payload() {
    let (client, mut server) = tokio::io::duplex(64);
    let mut stream = DeferredVlessResponseStream::new(client);

    server
        .write_all(&[
            VLESS_VERSION,
            0x03,
            b'a',
            b'b',
            b'c',
            b'p',
            b'o',
            b'n',
            b'g',
        ])
        .await
        .expect("write response");

    let mut payload = [0_u8; 4];
    stream.read_exact(&mut payload).await.expect("read payload");

    assert_eq!(&payload, b"pong");
}

#[tokio::test]
async fn deferred_vless_response_forwards_writes_before_response_arrives() {
    let (client, mut server) = tokio::io::duplex(64);
    let mut stream = DeferredVlessResponseStream::new(client);

    stream.write_all(b"ping").await.expect("write request");

    let mut request = [0_u8; 4];
    server.read_exact(&mut request).await.expect("read request");

    assert_eq!(&request, b"ping");
}
