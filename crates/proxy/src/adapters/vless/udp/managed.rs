use tokio::task::JoinSet;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::udp_flow::managed::{ManagedStreamConnectionSend, ManagedStreamPacketSender};
use crate::runtime::udp_flow::packet_path::ChainTask;

mod establish;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_flow(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &crate::runtime::Proxy,
    mux_pool: &vless::mux_pool::MuxConnectionPool,
    session: &Session,
    server: &str,
    port: u16,
    config: vless::udp::VlessUdpFlowConfig<'_>,
    transport: crate::transport::VlessUdpTransportOptions<'_>,
    payload: &[u8],
) -> Result<(), EngineError> {
    if establish::start_mux_fast_path(
        proxy, mux_pool, session, server, port, config, transport, payload,
    )
    .await?
    {
        return Ok(());
    }

    upstreams
        .send_or_insert_target(
            &session.target,
            session.port,
            ManagedStreamConnectionSend {
                chain_tasks,
                proxy,
                target: &session.target,
                port: session.port,
                payload,
            },
            establish::direct_flow(proxy, session, server, port, config, transport, payload),
        )
        .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_relay_two_stream(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    post_carrier: crate::transport::RelayCarrier,
    get_carrier: crate::transport::RelayCarrier,
    config: vless::udp::VlessUdpFlowConfig<'_>,
    split_http: &zero_config::SplitHttpConfig,
    payload: &[u8],
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vless_split_http_over_relay(
        post_carrier.stream,
        get_carrier.stream,
        split_http,
    )
    .await?;
    let upstream = establish::over_stream(proxy, session, config, payload, stream).await?;
    upstreams.insert_and_bridge_target(session.target.clone(), session.port, chain_tasks, upstream);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_relay_final_hop(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    config: vless::udp::VlessUdpFlowConfig<'_>,
    transport: crate::transport::VlessUdpTransportOptions<'_>,
    payload: &[u8],
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vless_outbound_transport_over_stream(
        crate::transport::VlessFinalHopTransportRequest {
            carrier,
            options: crate::transport::VlessTransportOptions {
                tls: transport.tls,
                reality: transport.reality,
                ws: transport.ws,
                grpc: transport.grpc,
                h2: transport.h2,
                http_upgrade: transport.http_upgrade,
                split_http: transport.split_http,
                source_dir: transport.source_dir,
            },
        },
    )
    .await?;
    let upstream = establish::over_stream(proxy, session, config, payload, stream).await?;
    upstreams.insert_and_bridge_target(session.target.clone(), session.port, chain_tasks, upstream);
    Ok(())
}
