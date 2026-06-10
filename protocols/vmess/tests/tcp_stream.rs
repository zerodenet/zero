use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use vmess::{parse_uuid, VmessAeadStream, VmessCipher, VmessInbound, VmessOutbound, VmessUser};
use zero_traits::UdpPacketFraming;

struct TestSocket(DuplexStream);

impl AsyncSocket for TestSocket {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(&mut self.0, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(&mut self.0, buf).await?;
        AsyncWriteExt::flush(&mut self.0).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(&mut self.0).await
    }
}

impl AsyncRead for TestSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TestSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn tcp_stream_encrypts_body_in_both_directions_for_all_ciphers() {
    for cipher in [
        VmessCipher::Aes128Gcm,
        VmessCipher::Chacha20Poly1305,
        VmessCipher::None,
        VmessCipher::Zero,
    ] {
        roundtrip_cipher(cipher).await;
    }
}

#[tokio::test]
async fn tcp_stream_shutdown_sends_body_termination_for_all_ciphers() {
    for cipher in [
        VmessCipher::Aes128Gcm,
        VmessCipher::Chacha20Poly1305,
        VmessCipher::None,
        VmessCipher::Zero,
    ] {
        shutdown_roundtrip_cipher(cipher).await;
    }
}

#[test]
fn cipher_auto_maps_to_aead_baseline() {
    assert_eq!(VmessCipher::from_name("auto"), Some(VmessCipher::Aes128Gcm));
}

#[test]
fn udp_packet_framing_roundtrips_domain_target() {
    let target = Address::Domain("example.com".to_owned());
    let payload = b"vmess udp payload";
    let encoded =
        <VmessOutbound as UdpPacketFraming<vmess::VmessUdpPacketTarget>>::encode_udp_packet(
            &VmessOutbound,
            &vmess::VmessUdpPacketTarget {
                address: &target,
                port: 53,
                payload,
            },
        )
        .expect("encode vmess udp packet");

    let decoded =
        <VmessOutbound as UdpPacketFraming<vmess::VmessUdpPacketTarget>>::decode_udp_packet(
            &VmessOutbound,
            &encoded,
        )
        .expect("decode vmess udp packet");

    assert_eq!(decoded.target, target);
    assert_eq!(decoded.port, 53);
    assert_eq!(decoded.payload, payload);
}

async fn roundtrip_cipher(cipher: VmessCipher) {
    let uuid = parse_uuid("11111111-2222-3333-4444-555555555555").expect("uuid");
    let (client_io, server_io) = tokio::io::duplex(128 * 1024);
    let target_session = Session::new(
        0,
        Address::Domain("example.com".to_owned()),
        443,
        Network::Tcp,
        ProtocolType::Vmess,
    );
    let upload = build_payload(37_000);
    let download = build_payload(21_000);

    let server_upload = upload.clone();
    let server_download = download.clone();
    let server = tokio::spawn(async move {
        let mut socket = TestSocket(server_io);
        let accepted = VmessInbound
            .accept_tcp(
                &mut socket,
                &VmessUser {
                    id: uuid,
                    cipher,
                    credential_id: None,
                    principal_key: None,
                    up_bps: None,
                    down_bps: None,
                },
            )
            .await
            .expect("server accept");

        assert_eq!(
            accepted.session.target,
            Address::Domain("example.com".to_owned())
        );
        assert_eq!(accepted.session.port, 443);

        let mut stream = VmessAeadStream::inbound(socket, accepted).expect("server stream");
        let mut received = vec![0_u8; server_upload.len()];
        stream
            .read_exact(&mut received)
            .await
            .expect("server read upload");
        assert_eq!(received, server_upload);

        stream
            .write_all(&server_download)
            .await
            .expect("server write download");
        stream.flush().await.expect("server flush");
    });

    let mut socket = TestSocket(client_io);
    let outbound_session = VmessOutbound
        .establish_tcp_session(&mut socket, &target_session, &uuid, cipher)
        .await
        .expect("client handshake");
    let mut stream = VmessAeadStream::outbound(socket, outbound_session).expect("client stream");

    stream
        .write_all(&upload)
        .await
        .expect("client write upload");
    stream.flush().await.expect("client flush");

    let mut received = vec![0_u8; download.len()];
    stream
        .read_exact(&mut received)
        .await
        .expect("client read download");
    assert_eq!(received, download);

    server.await.expect("server task");
}

async fn shutdown_roundtrip_cipher(cipher: VmessCipher) {
    let uuid = parse_uuid("11111111-2222-3333-4444-555555555555").expect("uuid");
    let (client_io, server_io) = tokio::io::duplex(128 * 1024);
    let target_session = Session::new(
        0,
        Address::Domain("example.com".to_owned()),
        443,
        Network::Tcp,
        ProtocolType::Vmess,
    );
    let upload = build_payload(8192);

    let server_upload = upload.clone();
    let server = tokio::spawn(async move {
        let mut socket = TestSocket(server_io);
        let accepted = VmessInbound
            .accept_tcp(
                &mut socket,
                &VmessUser {
                    id: uuid,
                    cipher,
                    credential_id: None,
                    principal_key: None,
                    up_bps: None,
                    down_bps: None,
                },
            )
            .await
            .expect("server accept");

        let mut stream = VmessAeadStream::inbound(socket, accepted).expect("server stream");
        let mut received = Vec::new();
        stream
            .read_to_end(&mut received)
            .await
            .expect("server read to eof");
        assert_eq!(received, server_upload);
    });

    let mut socket = TestSocket(client_io);
    let outbound_session = VmessOutbound
        .establish_tcp_session(&mut socket, &target_session, &uuid, cipher)
        .await
        .expect("client handshake");
    let mut stream = VmessAeadStream::outbound(socket, outbound_session).expect("client stream");

    stream
        .write_all(&upload)
        .await
        .expect("client write upload");
    stream.shutdown().await.expect("client shutdown");

    server.await.expect("server task");
}

fn build_payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}
