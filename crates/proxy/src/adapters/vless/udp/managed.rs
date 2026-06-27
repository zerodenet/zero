use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::managed::{ManagedStreamConnectionSend, ManagedStreamPacketSender};
use crate::runtime::udp_flow::packet_path::ChainTask;

mod establish;
mod model;

pub(crate) use model::{VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow};

pub(crate) async fn start_flow(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    request: VlessUdpStartFlow<'_>,
) -> Result<(), EngineError> {
    if establish::start_mux_fast_path(&request).await? {
        return Ok(());
    }

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

pub(crate) async fn start_relay_two_stream(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    request: VlessUdpRelayTwoStream<'_>,
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vless_split_http_over_relay(
        request.post_carrier.stream,
        request.get_carrier.stream,
        request.split_http,
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

pub(crate) async fn start_relay_final_hop(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    request: VlessUdpRelayFinalHopStart<'_>,
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vless_outbound_transport_over_stream(
        crate::transport::VlessFinalHopTransportRequest {
            carrier: request.carrier,
            options: crate::transport::VlessTransportOptions {
                tls: request.transport.tls,
                reality: request.transport.reality,
                ws: request.transport.ws,
                grpc: request.transport.grpc,
                h2: request.transport.h2,
                http_upgrade: request.transport.http_upgrade,
                split_http: request.transport.split_http,
                source_dir: request.transport.source_dir,
            },
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
