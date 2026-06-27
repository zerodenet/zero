use super::connect;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<mieru::MieruUdpFlowSession, EngineError> {
    let stream = connect::direct_stream(proxy, endpoint).await?;
    packet_stream(stream, resume).await
}

pub(super) async fn packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<mieru::MieruUdpFlowSession, EngineError> {
    mieru::establish_udp_flow_with_resume(stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })
}
