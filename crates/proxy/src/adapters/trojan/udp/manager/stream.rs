use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) session: trojan::TrojanUdpFlowSession,
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

    let session = trojan::TrojanUdpFlowSession::new(trojan::spawn_udp_flow(stream));

    Ok(PacketStream { session })
}
