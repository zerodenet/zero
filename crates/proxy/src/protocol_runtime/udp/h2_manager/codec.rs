use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::UdpDatagramFraming;

pub(super) fn packet(
    target: &Address,
    target_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, EngineError> {
    <hysteria2::Hysteria2Outbound as UdpDatagramFraming<
        hysteria2::Hysteria2UdpPacketTarget<'_>,
        (),
    >>::encode_udp_datagram(
        &hysteria2::Hysteria2Outbound,
        &hysteria2::Hysteria2UdpPacketTarget {
            session_id: 0,
            packet_id: 0,
            target,
            port: target_port,
            payload,
        },
    )
    .map_err(EngineError::from)
}

pub(super) fn decode_packet(payload: &[u8]) -> Result<hysteria2::Hysteria2UdpPacket, EngineError> {
    <hysteria2::Hysteria2Outbound as UdpDatagramFraming<
        hysteria2::Hysteria2UdpPacketTarget<'_>,
        (),
    >>::decode_udp_datagram(&hysteria2::Hysteria2Outbound, &(), payload)
    .map_err(EngineError::from)
}
