use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::managed::{ManagedStreamConnectionSend, ManagedStreamPacketSender};
use crate::runtime::udp_flow::packet_path::ChainTask;

mod establish;
mod model;

pub(crate) use model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};

pub(crate) async fn start_flow(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    request: VmessUdpStartFlow<'_>,
) -> Result<(), EngineError> {
    upstreams
        .send_or_insert_target(
            &request.session.target,
            request.session.port,
            ManagedStreamConnectionSend {
                chain_tasks,
                proxy: request.proxy,
                target: &request.session.target,
                port: request.session.port,
                payload: request.payload,
            },
            establish::direct_flow(&request),
        )
        .await
}

pub(crate) async fn start_relay_flow(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    request: VmessUdpRelayFlowStart<'_>,
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vmess_outbound_transport_over_stream(
        crate::transport::VmessFinalHopTransportRequest {
            carrier: request.carrier,
            options: request.transport,
        },
    )
    .await?;
    let upstream = establish::over_stream(
        request.proxy,
        request.session,
        request.config,
        request.payload,
        stream,
    )
    .await?;
    upstreams.insert_and_bridge_target(
        request.session.target.clone(),
        request.session.port,
        chain_tasks,
        upstream,
    );
    Ok(())
}
