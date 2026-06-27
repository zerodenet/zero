use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) session: mieru::MieruUdpFlowSession,
}

pub(super) async fn spawn_packet_stream(
    mut stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let flow_io = mieru::MieruUdpFlowIo::establish_with_resume(&mut stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    let session = mieru::MieruUdpFlowSession::new(mieru::spawn_udp_flow(stream, flow_io));
    Ok(PacketStream { session })
}
