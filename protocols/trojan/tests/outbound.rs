#![cfg(feature = "crypto")]

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;
use trojan::inbound::{TrojanInbound, TrojanInboundProfile};
use trojan::outbound::{PreparedTrojanOutboundRequestBundle, TrojanOutbound};
use trojan::udp::TrojanUdpPacket;
use zero_core::{
    Address, Error, InboundStreamRoute, InboundStreamUdpRelay, Network, ProtocolType, Session,
    StreamUdpResponder,
};
use zero_traits::{AsyncSocket, ClientTlsProfile, UdpPacketStreamFraming};

const CMD_TCP: u8 = 0x01;
const CMD_UDP: u8 = 0x03;
const ATYP_DOMAIN: u8 = 0x03;
const CRLF: &[u8] = b"\r\n";
const PASSWORD_HASH_LEN: usize = 56;

#[derive(Default)]
struct RecordingSocket {
    writes: Vec<Vec<u8>>,
    read_buf: Vec<u8>,
    read_offset: usize,
}

#[derive(Debug)]
enum TestOpenError {
    Protocol,
    Io,
}

impl From<Error> for TestOpenError {
    fn from(_value: Error) -> Self {
        Self::Protocol
    }
}

impl From<io::Error> for TestOpenError {
    fn from(_value: io::Error) -> Self {
        Self::Io
    }
}

impl AsyncSocket for RecordingSocket {
    type Error = io::Error;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.read_offset >= self.read_buf.len() {
            return Ok(0);
        }
        let n = _buf
            .len()
            .min(self.read_buf.len().saturating_sub(self.read_offset));
        _buf[..n].copy_from_slice(&self.read_buf[self.read_offset..self.read_offset + n]);
        self.read_offset += n;
        Ok(n)
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            self.writes.push(buf.to_vec());
            Ok(())
        }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { Ok(()) }
    }
}

impl AsyncWrite for RecordingSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.writes.push(buf.to_vec());
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
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
    let request =
        PreparedTrojanOutboundRequestBundle::from_config("test-password", None, false, None);
    let (socket, written_len) = request
        .open_tcp_stream_with_transport(&session, |_| async {
            Ok::<RecordingSocket, TestOpenError>(RecordingSocket::default())
        })
        .await
        .expect("open trojan request")
        .into_parts();

    assert_eq!(socket.writes.len(), 1);
    let request = &socket.writes[0];
    assert_eq!(written_len, request.len() as u64);
    assert_eq!(&request[PASSWORD_HASH_LEN..PASSWORD_HASH_LEN + 2], CRLF);
    assert_eq!(request[PASSWORD_HASH_LEN + 2], CMD_TCP);
    assert_eq!(request[PASSWORD_HASH_LEN + 3], ATYP_DOMAIN);
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

#[tokio::test]
async fn outbound_establishes_udp_packet_tunnel() {
    let session = Session::new(
        0,
        Address::Domain("dns.google".to_owned()),
        53,
        Network::Udp,
        ProtocolType::Trojan,
    );
    let mut socket = RecordingSocket::default();

    trojan::udp::establish_udp_packet_tunnel(&mut socket, &session, "test-password")
        .await
        .expect("establish trojan udp tunnel");

    assert_eq!(socket.writes.len(), 1);
    let request = &socket.writes[0];
    assert_eq!(&request[PASSWORD_HASH_LEN..PASSWORD_HASH_LEN + 2], CRLF);
    assert_eq!(request[PASSWORD_HASH_LEN + 2], CMD_UDP);
    assert_eq!(request[PASSWORD_HASH_LEN + 3], ATYP_DOMAIN);
    assert_eq!(request[PASSWORD_HASH_LEN + 4] as usize, "dns.google".len());
    assert_eq!(
        &request[PASSWORD_HASH_LEN + 5..PASSWORD_HASH_LEN + 15],
        b"dns.google"
    );
    assert_eq!(
        u16::from_be_bytes([
            request[PASSWORD_HASH_LEN + 15],
            request[PASSWORD_HASH_LEN + 16]
        ]),
        53
    );
    assert_eq!(
        &request[PASSWORD_HASH_LEN + 17..PASSWORD_HASH_LEN + 19],
        CRLF
    );
}

#[tokio::test]
async fn udp_stream_framing_roundtrips_packet() {
    let packet = TrojanUdpPacket::new(Address::Ipv4([8, 8, 8, 8]), 53, b"query".to_vec());
    let mut writer = RecordingSocket::default();

    <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::write_udp_packet(
        &TrojanOutbound,
        &mut writer,
        &packet,
    )
    .await
    .expect("write trojan udp packet");

    assert_eq!(writer.writes.len(), 1);
    let body_len = u16::from_be_bytes([writer.writes[0][0], writer.writes[0][1]]) as usize;
    assert_eq!(body_len, writer.writes[0].len() - 2);

    let mut reader = RecordingSocket {
        read_buf: writer.writes[0].clone(),
        ..RecordingSocket::default()
    };
    let decoded = <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::read_udp_packet(
        &TrojanOutbound,
        &mut reader,
    )
    .await
    .expect("read trojan udp packet");

    assert_eq!(decoded, packet);
}

#[tokio::test]
async fn inbound_udp_helpers_roundtrip_response_packet() {
    let password = "test-password";
    let session = Session::new(
        0,
        Address::Domain("dns.example".to_owned()),
        5353,
        Network::Udp,
        ProtocolType::Trojan,
    );
    let mut handshake_writer = RecordingSocket::default();
    trojan::udp::establish_udp_packet_tunnel(&mut handshake_writer, &session, password)
        .await
        .expect("write trojan udp handshake");

    let route = TrojanInboundProfile::from_config_password(password)
        .accept_client_owned(
            TrojanInbound,
            RecordingSocket {
                read_buf: handshake_writer.writes[0].clone(),
                ..RecordingSocket::default()
            },
        )
        .await
        .expect("accept trojan udp route");

    InboundStreamRoute::dispatch_inbound_route(
        route,
        |_, _| async { panic!("expected trojan udp route") },
        |_, relay| async move {
            let (_, mut udp_responder, _) = relay.into_stream_udp_parts();
            let mut writer = RecordingSocket::default();

            StreamUdpResponder::write_response_for_target(
                &mut udp_responder,
                &mut writer,
                &Address::Domain("dns.example".to_owned()),
                5353,
                b"answer",
            )
            .await
            .expect("write trojan udp response");

            let mut reader = RecordingSocket {
                read_buf: writer.writes[0].clone(),
                ..RecordingSocket::default()
            };
            let decoded =
                StreamUdpResponder::read_inbound_dispatch(&mut udp_responder, &mut reader)
                    .await
                    .expect("read trojan udp packet")
                    .expect("decoded trojan udp packet");

            assert_eq!(decoded.target(), &Address::Domain("dns.example".to_owned()));
            assert_eq!(decoded.port(), 5353);
            assert_eq!(decoded.payload(), b"answer");
            Ok::<(), Error>(())
        },
    )
    .await
    .expect("dispatch trojan udp route");
}

#[test]
fn tcp_connect_config_exposes_profile_accessors() {
    let request = PreparedTrojanOutboundRequestBundle::from_config(
        "test-password",
        Some("edge.example"),
        true,
        Some("chrome"),
    );
    let tls_profile = request.owned_tls_profile();

    assert_eq!(
        ClientTlsProfile::server_name(&tls_profile),
        Some("edge.example")
    );
    assert!(ClientTlsProfile::insecure(&tls_profile));
    assert_eq!(
        ClientTlsProfile::client_fingerprint(&tls_profile),
        Some("chrome")
    );
}

#[test]
fn udp_flow_resume_tls_profile_uses_fallback_server_name() {
    let plan = PreparedTrojanOutboundRequestBundle::from_config(
        "test-password",
        None,
        false,
        Some("chrome"),
    )
    .udp_direct_flow_plan();
    let tls_profile = plan.owned_tls_profile(Some("fallback.example"));

    assert_eq!(
        ClientTlsProfile::server_name(&tls_profile),
        Some("fallback.example")
    );
    assert!(!ClientTlsProfile::insecure(&tls_profile));
    assert_eq!(
        ClientTlsProfile::client_fingerprint(&tls_profile),
        Some("chrome")
    );
}
