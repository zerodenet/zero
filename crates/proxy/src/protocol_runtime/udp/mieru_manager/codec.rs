use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::UdpPacketFraming;

pub(super) fn packet(
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    let packet =
        socks5::build_udp_packet(target, target_port, payload).map_err(EngineError::from)?;
    encode_associate_packet(&packet)
}

fn encode_associate_packet(payload: &[u8]) -> Result<Vec<u8>, EngineError> {
    <mieru::MieruProtocol as UdpPacketFraming<mieru::MieruUdpAssociatePacket<'_>>>::encode_udp_packet(
        &mieru::MieruProtocol,
        &mieru::MieruUdpAssociatePacket { payload },
    )
    .map_err(EngineError::from)
}

pub(super) fn decode_associate_packet(
    payload: &[u8],
) -> Result<mieru::MieruUdpAssociatePayload, EngineError> {
    <mieru::MieruProtocol as UdpPacketFraming<mieru::MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
        &mieru::MieruProtocol,
        payload,
    )
    .map_err(EngineError::from)
}
