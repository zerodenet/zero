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
    stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let session = trojan::establish_udp_flow_with_resume(stream, session, resume).await?;

    Ok(PacketStream { session })
}
