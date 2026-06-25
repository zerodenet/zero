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

#[test]
fn udp_response_encoding_wraps_packet_mode_and_preserves_raw_mode() {
    let target = Address::Domain("example.com".to_owned());
    let packet = vmess::encode_udp_response(
        vmess::VmessUdpPayloadMode::VmessPacket,
        &target,
        5353,
        b"dns",
    )
    .expect("encode packet response");
    let decoded = vmess::parse_udp_packet(&packet).expect("decode packet response");
    assert_eq!(decoded.target, target);
    assert_eq!(decoded.port, 5353);
    assert_eq!(decoded.payload, b"dns");

    let raw = vmess::encode_udp_response(
        vmess::VmessUdpPayloadMode::RawDatagram,
        &Address::Ipv4([127, 0, 0, 1]),
        53,
        b"raw",
    )
    .expect("encode raw response");
    assert_eq!(raw, b"raw");
}

#[test]
fn inbound_udp_payload_decoder_detects_packet_mode_then_requires_packets() {
    let default_target = Address::Domain("fallback.example".to_owned());
    let packet =
        vmess::build_udp_packet(&Address::Domain("packet.example".to_owned()), 5353, b"dns")
            .expect("build packet");
    let decoded = vmess::decode_inbound_udp_payload(
        vmess::VmessUdpPayloadState::Unknown,
        &default_target,
        53,
        &packet,
    )
    .expect("decode packet payload");
    assert_eq!(
        decoded.state,
        vmess::VmessUdpPayloadState::Mode(vmess::VmessUdpPayloadMode::VmessPacket)
    );
    assert_eq!(decoded.target, Address::Domain("packet.example".to_owned()));
    assert_eq!(decoded.port, 5353);
    assert_eq!(decoded.payload, b"dns");

    assert!(vmess::decode_inbound_udp_payload(decoded.state, &default_target, 53, b"raw").is_err());
}

#[test]
fn inbound_udp_payload_decoder_falls_back_to_raw_mode() {
    let default_target = Address::Ipv4([10, 0, 0, 1]);
    let decoded = vmess::decode_inbound_udp_payload(
        vmess::VmessUdpPayloadState::Unknown,
        &default_target,
        9999,
        b"raw",
    )
    .expect("decode raw payload");
    assert_eq!(
        decoded.state,
        vmess::VmessUdpPayloadState::Mode(vmess::VmessUdpPayloadMode::RawDatagram)
    );
    assert_eq!(decoded.target, default_target);
    assert_eq!(decoded.port, 9999);
    assert_eq!(decoded.payload, b"raw");
}

#[tokio::test]
async fn mux_udp_response_encoding_wraps_packet_mode_before_mux_frame() {
    let target = Address::Ipv4([8, 8, 8, 8]);
    let frame = vmess::encode_mux_udp_response(
        7,
        vmess::VmessUdpPayloadMode::VmessPacket,
        &target,
        53,
        b"query",
    )
    .expect("encode mux udp response");
    let (client, server) = tokio::io::duplex(1024);
    let write = tokio::spawn(async move {
        let mut client = client;
        client.write_all(&frame).await.expect("write mux frame");
    });
    let mut server = TestSocket(server);
    let decoded = vmess::read_mux_frame(&mut server)
        .await
        .expect("decode mux frame");
    assert_eq!(decoded.session_id, 7);
    write.await.expect("writer task");
    let packet = vmess::parse_udp_packet(&decoded.payload).expect("decode mux udp payload");
    assert_eq!(packet.target, target);
    assert_eq!(packet.port, 53);
    assert_eq!(packet.payload, b"query");
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
