use zero_core::Address;
use zero_engine::EngineError;

pub(super) fn encode_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
    cipher: shadowsocks::CipherKind,
    password: &str,
) -> Result<Vec<u8>, EngineError> {
    shadowsocks::encode_udp_datagram(target, port, payload, cipher, password.as_bytes())
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))
}

pub(super) fn decode_packet(
    payload: &[u8],
    cipher: shadowsocks::CipherKind,
    password: &str,
) -> Result<shadowsocks::ShadowsocksUdpPacket, EngineError> {
    shadowsocks::decode_udp_datagram(payload, cipher, password.as_bytes())
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))
}
