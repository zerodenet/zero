//! Trojan inbound protocol handler.

use zero_core::{Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use super::outbound::TrojanUdpPacket;
use super::shared::{read_password, read_request, CMD_TCP, CMD_UDP};

/// Trojan inbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInbound;

/// Result of accepting a Trojan connection.
pub struct TrojanAccept {
    pub session: Session,
    pub command: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundUdpRequest {
    target: zero_core::Address,
    port: u16,
    payload: Vec<u8>,
}

impl TrojanInboundUdpRequest {
    fn from_packet(packet: TrojanUdpPacket) -> Self {
        Self {
            target: packet.target,
            port: packet.port,
            payload: packet.payload,
        }
    }

    pub fn target(&self) -> &zero_core::Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInboundUdpSession {
    codec: TrojanInboundUdpCodec,
}

impl TrojanInboundUdpSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read_request<S>(&self, stream: &mut S) -> Result<TrojanInboundUdpRequest, Error>
    where
        S: AsyncSocket,
    {
        self.codec
            .read_packet(stream)
            .await
            .map(TrojanInboundUdpRequest::from_packet)
    }

    pub async fn write_response<S>(
        &self,
        stream: &mut S,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.codec
            .write_response(stream, target, port, payload)
            .await
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInboundUdpCodec;

impl TrojanInboundUdpCodec {
    pub async fn read_packet<S>(&self, stream: &mut S) -> Result<TrojanUdpPacket, Error>
    where
        S: AsyncSocket,
    {
        let (target, port, payload) = super::shared::read_udp_packet(stream).await?;
        Ok(TrojanUdpPacket::new(target, port, payload))
    }

    pub async fn write_response<S>(
        &self,
        stream: &mut S,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        super::shared::write_udp_packet(stream, target, port, payload).await
    }
}

impl TrojanInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Accept a Trojan TCP connection.
    ///
    /// Reads password hash + command + target address from the stream.
    /// The password is validated against `passwords` (hex SHA224 hashes).
    pub async fn accept<S: AsyncSocket>(
        &self,
        stream: &mut S,
        passwords: &[String],
    ) -> Result<TrojanAccept, Error> {
        let hex = read_password(stream).await?;

        // Validate password.
        if !passwords.iter().any(|p| {
            #[cfg(feature = "crypto")]
            {
                use sha2::{Digest, Sha224};
                hex == super::shared::hex::encode(&Sha224::digest(p.as_bytes()))
            }
            #[cfg(not(feature = "crypto"))]
            {
                let _ = p;
                false
            }
        }) {
            return Err(Error::Protocol("trojan: invalid password"));
        }

        let (cmd, addr, port) = read_request(stream).await?;

        let network = match cmd {
            CMD_TCP => Network::Tcp,
            CMD_UDP => Network::Udp,
            _ => return Err(Error::Protocol("trojan: unsupported command")),
        };

        Ok(TrojanAccept {
            session: Session::new(0, addr, port, network, ProtocolType::Trojan),
            command: cmd,
        })
    }
}
