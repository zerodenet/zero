use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::UdpDatagramFraming;

pub(super) fn encode_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
    cipher: shadowsocks::CipherKind,
    password: &str,
) -> Result<Vec<u8>, EngineError> {
    <shadowsocks::ShadowsocksOutbound as UdpDatagramFraming<
        shadowsocks::ShadowsocksUdpPacketTarget,
        shadowsocks::ShadowsocksUdpDecodeContext,
    >>::encode_udp_datagram(
        &shadowsocks::ShadowsocksOutbound,
        &shadowsocks::ShadowsocksUdpPacketTarget {
            target,
            port,
            payload,
            cipher,
            password: password.as_bytes(),
        },
    )
    .map_err(|error| EngineError::Io(std::io::Error::other(error)))
}

pub(super) fn decode_packet(
    payload: &[u8],
    cipher: shadowsocks::CipherKind,
    password: &str,
) -> Result<shadowsocks::ShadowsocksUdpPacket, EngineError> {
    <shadowsocks::ShadowsocksOutbound as UdpDatagramFraming<
        shadowsocks::ShadowsocksUdpPacketTarget,
        shadowsocks::ShadowsocksUdpDecodeContext,
    >>::decode_udp_datagram(
        &shadowsocks::ShadowsocksOutbound,
        &shadowsocks::ShadowsocksUdpDecodeContext {
            cipher,
            password: password.as_bytes(),
        },
        payload,
    )
    .map_err(|error| EngineError::Io(std::io::Error::other(error)))
}
