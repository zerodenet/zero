use alloc::vec::Vec;

use zero_core::{Error, ProtocolType, Session};
#[cfg(feature = "reality")]
use zero_traits::DeferredTcpTunnelProtocol;
use zero_traits::{AsyncSocket, TcpTunnelProtocol, UdpPacketFraming, UdpPacketTunnelProtocol};

#[cfg(feature = "reality")]
use crate::flow::flow_build_request;
use crate::mux::MuxClient;
use crate::shared::{
    build_udp_packet, parse_udp_packet, read_addon, read_exact, write_address, CMD_MUX,
    VLESS_VERSION,
};
use crate::VlessUdpPacket;

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessOutbound;

impl VlessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
    }

    pub async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request(stream, session, id).await?;
        read_response(stream).await
    }

    #[cfg(feature = "reality")]
    pub async fn establish_tcp_tunnel_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, session, id, flow)
            .await?;
        read_response(stream).await
    }

    pub async fn send_tcp_request<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    #[cfg(feature = "reality")]
    pub async fn send_tcp_request_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request_with_flow(session, id, flow)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    pub async fn send_udp_request<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_udp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS UDP request"))
    }

    pub async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_udp_request(stream, session, id).await?;
        read_response(stream).await
    }

    /// Send VLESS MUX header and read server response.
    /// Returns a MuxClient for subsequent stream allocation.
    pub async fn establish_mux<S>(&self, stream: &mut S, id: &[u8; 16]) -> Result<MuxClient, Error>
    where
        S: AsyncSocket,
    {
        let request = build_mux_request(id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS MUX request"))?;
        read_response(stream).await?;
        Ok(MuxClient::new())
    }
}

/// Target parameters for VLESS TCP tunnel (non-flow path).
#[derive(Debug, Clone, Copy)]
pub struct VlessTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
}

impl<'a> TcpTunnelProtocol<VlessTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_tunnel(stream, target.session, target.id)
            .await
    }
}

/// Target parameters for VLESS TCP tunnel with flow (Vision/Reality path).
///
/// The flow parameter controls the XTLS Vision flow negotiation. When `None`,
/// the handshake uses the standard path. This target is only available when
/// the `reality` feature is enabled because flow handling requires the
/// Vision/Reality code path.
#[cfg(feature = "reality")]
#[derive(Debug, Clone, Copy)]
pub struct VlessFlowTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
    pub flow: Option<&'a str>,
}

#[cfg(feature = "reality")]
impl<'a> TcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        match target.flow {
            Some(f) => {
                self.establish_tcp_tunnel_with_flow(stream, target.session, target.id, Some(f))
                    .await
            }
            None => {
                self.establish_tcp_tunnel(stream, target.session, target.id)
                    .await
            }
        }
    }
}

#[cfg(feature = "reality")]
impl<'a> DeferredTcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn send_deferred_tcp_tunnel_request<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, target.session, target.id, target.flow)
            .await
    }
}

/// Target parameters for VLESS UDP packet tunnel over a connected stream.
#[derive(Debug, Clone, Copy)]
pub struct VlessUdpPacketTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
}

impl<'a> UdpPacketTunnelProtocol<VlessUdpPacketTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessUdpPacketTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_udp_packet_tunnel(stream, target.session, target.id)
            .await
    }
}

/// One UDP datagram to encode for a VLESS UDP packet tunnel.
#[derive(Debug, Clone, Copy)]
pub struct VlessUdpPacketTarget<'a> {
    pub address: &'a zero_core::Address,
    pub port: u16,
    pub payload: &'a [u8],
}

impl<'a> UdpPacketFraming<VlessUdpPacketTarget<'a>> for VlessOutbound {
    type Error = Error;
    type Decoded = VlessUdpPacket;

    fn encode_udp_packet(&self, packet: &VlessUdpPacketTarget<'a>) -> Result<Vec<u8>, Self::Error> {
        build_udp_packet(packet.address, packet.port, packet.payload)
    }

    fn decode_udp_packet(&self, packet: &[u8]) -> Result<Self::Decoded, Self::Error> {
        parse_udp_packet(packet)
    }
}

fn build_tcp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_TCP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

    Ok(request)
}

fn build_udp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_UDP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

    Ok(request)
}

fn build_mux_request(id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(CMD_MUX);
    // Dummy target — ignored by the MUX server
    request.extend_from_slice(&0u16.to_be_bytes());
    request.push(0x01); // ATYP_IPV4
    request.extend_from_slice(&[0u8; 4]);

    Ok(request)
}

#[cfg(feature = "reality")]
fn build_tcp_request_with_flow(
    session: &Session,
    id: &[u8; 16],
    flow: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let (fbyte, payload) = flow_build_request(
        id,
        flow,
        crate::shared::CMD_TCP,
        session.port,
        &session.target,
    )?;

    let mut request = Vec::with_capacity(24 + payload.len());
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(fbyte);
    request.extend_from_slice(&payload);

    Ok(request)
}

async fn read_response<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS response version"));
    }

    read_addon(stream).await
}
