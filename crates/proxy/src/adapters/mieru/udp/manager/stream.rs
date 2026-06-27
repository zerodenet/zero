use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) session: mieru::MieruUdpFlowSession,
}

pub(super) async fn spawn_packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let session = mieru::establish_udp_flow_with_resume(stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    Ok(PacketStream { session })
}
