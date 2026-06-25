use vmess::{VmessAccept, VmessAeadStream};
use zero_core::Address;
use zero_engine::EngineError;

use crate::transport::TcpRelayStream;

#[derive(Clone, Copy)]
pub(crate) enum VmessUdpPayloadMode {
    Unknown,
    VmessPacket,
    RawDatagram,
}

impl VmessUdpPayloadMode {
    fn protocol_mode(self) -> vmess::VmessUdpPayloadMode {
        match self {
            Self::Unknown | Self::VmessPacket => vmess::VmessUdpPayloadMode::VmessPacket,
            Self::RawDatagram => vmess::VmessUdpPayloadMode::RawDatagram,
        }
    }
}

pub(crate) fn encode_vmess_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    vmess::encode_mux_udp_response(mux_session_id, mode.protocol_mode(), target, port, payload)
}

pub(crate) fn encode_vmess_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    vmess::encode_udp_response(mode.protocol_mode(), target, port, payload)
}

pub(crate) fn wrap_vmess_client(
    stream: TcpRelayStream,
    accepted: VmessAccept,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(VmessAeadStream::inbound(
        stream, accepted,
    )?))
}

pub(crate) fn remote_addr_to_socket(
    addr: Option<zero_traits::IpAddress>,
) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
