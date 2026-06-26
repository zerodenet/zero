use zero_core::Address;
use zero_engine::EngineError;

pub(super) fn packet(
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    hysteria2::encode_udp_flow_packet(target, target_port, payload).map_err(EngineError::from)
}

pub(super) fn decode_packet(payload: &[u8]) -> Result<hysteria2::Hysteria2UdpPacket, EngineError> {
    hysteria2::decode_udp_flow_packet(payload).map_err(EngineError::from)
}
