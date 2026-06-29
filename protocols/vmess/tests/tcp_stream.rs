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
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::encode_udp_packet(
            &VmessOutbound,
            &vmess::udp::VmessUdpPacketTarget {
                address: &target,
                port: 53,
                payload,
            },
        )
        .expect("encode vmess udp packet");

    let decoded =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::decode_udp_packet(
            &VmessOutbound,
            &encoded,
        )
        .expect("decode vmess udp packet");

    assert_eq!(decoded.target(), &target);
    assert_eq!(decoded.port(), 53);
    assert_eq!(decoded.payload(), payload);
}

#[tokio::test]
async fn udp_response_encoding_wraps_packet_mode_and_preserves_raw_mode() {
    let target = Address::Domain("example.com".to_owned());
    let default_target = Address::Domain("fallback.example".to_owned());
    let request =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::encode_udp_packet(
            &VmessOutbound,
            &vmess::udp::VmessUdpPacketTarget {
                address: &Address::Domain("packet.example".to_owned()),
                port: 5353,
                payload: b"dns",
            },
        )
        .expect("build packet");
    let mut udp_session = vmess::udp::VmessInboundUdpSession::new(default_target, 53);
    udp_session
        .decode_request(&request)
        .expect("enter packet response mode");

    let (mut client, mut server) = tokio::io::duplex(1024);
    udp_session
        .write_response_tokio(&mut client, &target, 5353, b"dns")
        .await
        .expect("encode packet response");
    let mut packet = Vec::new();
    client.shutdown().await.expect("shutdown writer");
    server
        .read_to_end(&mut packet)
        .await
        .expect("read packet response");
    let decoded =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::decode_udp_packet(
            &VmessOutbound,
            &packet,
        )
        .expect("decode packet response");
    assert_eq!(decoded.target(), &target);
    assert_eq!(decoded.port(), 5353);
    assert_eq!(decoded.payload(), b"dns");

    let mut raw_session =
        vmess::udp::VmessInboundUdpSession::new(Address::Ipv4([127, 0, 0, 1]), 53);
    raw_session
        .decode_request(b"raw")
        .expect("enter raw response mode");
    let (mut client, mut server) = tokio::io::duplex(1024);
    raw_session
        .write_response_tokio(&mut client, &Address::Ipv4([127, 0, 0, 1]), 53, b"raw")
        .await
        .expect("encode raw response");
    let mut raw = Vec::new();
    client.shutdown().await.expect("shutdown writer");
    server
        .read_to_end(&mut raw)
        .await
        .expect("read raw response");
    assert_eq!(raw, b"raw");
}

#[test]
fn inbound_udp_payload_decoder_detects_packet_mode_then_requires_packets() {
    let default_target = Address::Domain("fallback.example".to_owned());
    let packet =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::encode_udp_packet(
            &VmessOutbound,
            &vmess::udp::VmessUdpPacketTarget {
                address: &Address::Domain("packet.example".to_owned()),
                port: 5353,
                payload: b"dns",
            },
        )
        .expect("build packet");
    let mut udp_session = vmess::udp::VmessInboundUdpSession::new(default_target, 53);
    let decoded = udp_session
        .decode_request(&packet)
        .expect("decode packet payload");
    assert_eq!(
        decoded.target(),
        &Address::Domain("packet.example".to_owned())
    );
    assert_eq!(decoded.port(), 5353);
    assert_eq!(decoded.payload(), b"dns");

    assert!(udp_session.decode_request(b"raw").is_err());
}

#[test]
fn inbound_udp_payload_decoder_falls_back_to_raw_mode() {
    let default_target = Address::Ipv4([10, 0, 0, 1]);
    let mut udp_session = vmess::udp::VmessInboundUdpSession::new(default_target.clone(), 9999);
    let decoded = udp_session
        .decode_request(b"raw")
        .expect("decode raw payload");
    assert_eq!(decoded.target(), &default_target);
    assert_eq!(decoded.port(), 9999);
    assert_eq!(decoded.payload(), b"raw");
}

#[tokio::test]
async fn mux_udp_response_encoding_wraps_packet_mode_before_mux_frame() {
    let target = Address::Ipv4([8, 8, 8, 8]);
    let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let writer = vmess::mux::VmessInboundMuxWriter::new(write_tx);
    let request =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::encode_udp_packet(
            &VmessOutbound,
            &vmess::udp::VmessUdpPacketTarget {
                address: &target,
                port: 53,
                payload: b"query",
            },
        )
        .expect("build packet");
    let mut udp_session = vmess::udp::VmessInboundUdpSession::new(target.clone(), 53);
    udp_session
        .decode_request(&request)
        .expect("enter packet mux response mode");
    udp_session
        .write_mux_response(&writer, 7, &target, 53, b"query")
        .expect("encode mux udp response");
    let frame = write_rx.recv().await.expect("mux frame");
    let (client, server) = tokio::io::duplex(1024);
    let write = tokio::spawn(async move {
        let mut client = client;
        client.write_all(&frame).await.expect("write mux frame");
    });
    let mut server = TestSocket(server);
    let decoded = vmess::mux::read_frame(&mut server)
        .await
        .expect("decode mux frame");
    assert_eq!(decoded.session_id, 7);
    write.await.expect("writer task");
    let packet =
        <VmessOutbound as UdpPacketFraming<vmess::udp::VmessUdpPacketTarget>>::decode_udp_packet(
            &VmessOutbound,
            &decoded.payload,
        )
        .expect("decode mux udp payload");
    assert_eq!(packet.target(), &target);
    assert_eq!(packet.port(), 53);
    assert_eq!(packet.payload(), b"query");
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
