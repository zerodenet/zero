use vmess::{VmessAccept, VmessAeadStream};
use zero_engine::EngineError;

use crate::transport::TcpRelayStream;

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
