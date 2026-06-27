use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) sender: trojan::TrojanUdpFlowSender,
    pub(super) responses: trojan::TrojanUdpFlowResponses,
}

pub(super) async fn spawn_packet_stream(
    _proxy: &Proxy,
    session: &Session,
    mut stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let flow_io = trojan::TrojanUdpFlowIo;
    flow_io
        .establish_with_resume(&mut stream, session, resume)
        .await?;

    let trojan::TrojanUdpFlowHandle { sender, responses } = trojan::spawn_udp_flow(stream);

    Ok(PacketStream { sender, responses })
}
