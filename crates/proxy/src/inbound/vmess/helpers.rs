use vmess::{VmessAccept, VmessAeadStream};
use zero_core::Address;
use zero_engine::EngineError;

use crate::transport::TcpRelayStream;

#[derive(Clone, Copy)]
pub(crate) struct VmessUdpPayloadMode(vmess::VmessUdpPayloadState);

pub(crate) struct VmessInboundUdpPayload {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
}

impl VmessUdpPayloadMode {
    pub(crate) fn unknown() -> Self {
        Self(vmess::VmessUdpPayloadState::Unknown)
    }

    fn response_mode(self) -> vmess::VmessUdpPayloadMode {
        match self.0 {
            vmess::VmessUdpPayloadState::Unknown
            | vmess::VmessUdpPayloadState::Mode(vmess::VmessUdpPayloadMode::VmessPacket) => {
                vmess::VmessUdpPayloadMode::VmessPacket
            }
            vmess::VmessUdpPayloadState::Mode(vmess::VmessUdpPayloadMode::RawDatagram) => {
                vmess::VmessUdpPayloadMode::RawDatagram
            }
        }
    }

    fn update(&mut self, state: vmess::VmessUdpPayloadState) {
        self.0 = state;
    }
}

pub(crate) fn encode_vmess_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    vmess::VmessInboundUdpCodec.encode_mux_response(
        mux_session_id,
        mode.response_mode(),
        target,
        port,
        payload,
    )
}

pub(crate) fn encode_vmess_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    vmess::VmessInboundUdpCodec.encode_response(mode.response_mode(), target, port, payload)
}

pub(crate) fn decode_vmess_udp_payload(
    mode: &mut VmessUdpPayloadMode,
    default_target: &Address,
    default_port: u16,
    payload: &[u8],
) -> Result<VmessInboundUdpPayload, zero_core::Error> {
    let decoded = vmess::VmessInboundUdpCodec.decode_datagram(
        mode.0,
        default_target,
        default_port,
        payload,
    )?;
    mode.update(decoded.state);
    Ok(VmessInboundUdpPayload {
        target: decoded.target,
        port: decoded.port,
        payload: decoded.payload,
    })
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
