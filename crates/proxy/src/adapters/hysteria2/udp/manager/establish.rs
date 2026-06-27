use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::spawn_tuple_response_bridge;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use tokio::task::JoinSet;
use zero_engine::EngineError;

pub(super) async fn upstream(
    chain_tasks: &mut JoinSet<ChainTask>,
    session_id: u64,
    endpoint: OutboundEndpoint<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
    initial_packet: UdpPacketRef<'_>,
) -> Result<hysteria2::Hysteria2UdpFlowConnection, EngineError> {
    let session =
        crate::outbound::hysteria2::establish_udp_flow_session(endpoint, initial_packet, resume)
            .await?;

    spawn_tuple_response_bridge(
        chain_tasks,
        session.subscribe_responses(),
        session_id,
        "h2 upstream closed",
    );

    Ok(session)
}
