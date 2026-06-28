// Hysteria2 outbound protocol — outbound.rs

use alloc::vec::Vec;

use crate::shared::build_tcp_connect_header;
use crate::udp::{Hysteria2UdpPacket, Hysteria2UdpPacketTarget};
use zero_core::{Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, UdpDatagramFraming};

/// Hysteria2 outbound handler — sends auth and opens streams.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2Outbound;

impl Hysteria2Outbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Hysteria2
    }

    /// Send the authentication frame over a QUIC stream.
    pub async fn send_auth<S: AsyncSocket>(
        &self,
        stream: &mut S,
        hmac: &[u8; 32],
    ) -> Result<(), Error> {
        let frame = crate::shared::build_auth_frame(hmac);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("hysteria2: failed to write auth"))
    }

    /// Compute and send the QUIC-bound authentication frame.
    #[cfg(feature = "crypto")]
    pub async fn authenticate_with_salt<S: AsyncSocket>(
        &self,
        stream: &mut S,
        password: &str,
        salt: &[u8; 32],
    ) -> Result<(), Error> {
        let hmac = crate::shared::sign_hmac(password, salt);
        self.send_auth(stream, &hmac).await?;
        self.read_auth_response(stream).await
    }

    /// Read the authentication response from the server.
    pub async fn read_auth_response<S: AsyncSocket>(&self, stream: &mut S) -> Result<(), Error> {
        let mut buf = [0u8; 64];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|_| Error::Io("hysteria2: failed to read auth response"))?;
        if n == 0 {
            return Err(Error::Io("hysteria2: EOF reading auth response"));
        }
        crate::shared::parse_auth_response(&buf[..n])
    }

    /// Send a TCP connect request on a new stream.
    pub async fn send_tcp_connect<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<(), Error> {
        let header = build_tcp_connect_header(&session.target, session.port)?;
        stream
            .write_all(&header)
            .await
            .map_err(|_| Error::Io("hysteria2: failed to write connect header"))
    }

    /// Read the TCP connect response.
    pub async fn read_connect_response<S: AsyncSocket>(&self, stream: &mut S) -> Result<(), Error> {
        let mut buf = [0u8; 256];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|_| Error::Io("hysteria2: failed to read connect response"))?;
        if n == 0 {
            return Err(Error::Io("hysteria2: EOF reading connect response"));
        }
        if buf[0] != 0x01 {
            return Err(Error::Protocol("hysteria2: connect rejected"));
        }
        Ok(())
    }
}

impl<'a> UdpDatagramFraming<Hysteria2UdpPacketTarget<'a>, ()> for Hysteria2Outbound {
    type Error = Error;
    type Decoded = Hysteria2UdpPacket;

    fn encode_udp_datagram(
        &self,
        packet: &Hysteria2UdpPacketTarget<'a>,
    ) -> Result<Vec<u8>, Self::Error> {
        crate::udp::build_udp_datagram(
            packet.session_id,
            packet.packet_id,
            packet.target,
            packet.port,
            packet.payload,
        )
    }

    fn decode_udp_datagram(
        &self,
        _context: &(),
        datagram: &[u8],
    ) -> Result<Self::Decoded, Self::Error> {
        crate::udp::parse_udp_datagram(datagram)
    }
}
