use zero_core::Address;
use zero_engine::EngineError;

pub(super) fn packet(
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    mieru::encode_udp_flow_packet(target, target_port, payload).map_err(EngineError::from)
}

pub(super) fn decode_packet(payload: &[u8]) -> Result<mieru::MieruInboundUdpPacket, EngineError> {
    mieru::decode_udp_flow_packet(payload).map_err(EngineError::from)
}
