#![cfg(feature = "crypto")]

use std::io;

use trojan::{
    read_inbound_udp_packet, write_udp_response, TrojanOutbound, TrojanUdpPacket,
    TrojanUdpPacketTunnelTarget, CMD_TCP, CMD_UDP, CRLF, PASSWORD_HASH_LEN,
};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_traits::{AsyncSocket, UdpPacketStreamFraming, UdpPacketTunnelProtocol};

#[derive(Default)]
struct RecordingSocket {
    writes: Vec<Vec<u8>>,
    read_buf: Vec<u8>,
    read_offset: usize,
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
    assert_eq!(request[PASSWORD_HASH_LEN + 3], trojan::ATYP_DOMAIN);
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

    <TrojanOutbound as UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget>>::establish_udp_packet_tunnel(
        &TrojanOutbound,
        &mut socket,
        &TrojanUdpPacketTunnelTarget {
            session: &session,
            password: "test-password",
        },
    )
    .await
    .expect("establish trojan udp tunnel");

    assert_eq!(socket.writes.len(), 1);
    let request = &socket.writes[0];
    assert_eq!(&request[PASSWORD_HASH_LEN..PASSWORD_HASH_LEN + 2], CRLF);
    assert_eq!(request[PASSWORD_HASH_LEN + 2], CMD_UDP);
    assert_eq!(request[PASSWORD_HASH_LEN + 3], trojan::ATYP_DOMAIN);
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
    let packet = TrojanUdpPacket {
        target: Address::Ipv4([8, 8, 8, 8]),
        port: 53,
        payload: b"query".to_vec(),
    };
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
    let mut writer = RecordingSocket::default();

    write_udp_response(
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
    let decoded = read_inbound_udp_packet(&mut reader)
        .await
        .expect("read trojan udp packet");

    assert_eq!(decoded.target, Address::Domain("dns.example".to_owned()));
    assert_eq!(decoded.port, 5353);
    assert_eq!(decoded.payload, b"answer");
}
