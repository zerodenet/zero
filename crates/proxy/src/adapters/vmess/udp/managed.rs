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
    mux_pool: &vmess::mux::VmessMuxConnectionPool,
    session: &Session,
    server: &str,
    port: u16,
    config: vmess::udp::VmessUdpFlowConfig<'_>,
    mux_concurrency: Option<u32>,
    transport: crate::transport::VmessTransportOptions<'_>,
    payload: &[u8],
) -> Result<(), EngineError> {
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
            establish::direct_flow(
                proxy,
                mux_pool,
                session,
                server,
                port,
                config,
                mux_concurrency,
                transport,
                payload,
            ),
        )
        .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_relay_flow(
    upstreams: &mut ManagedStreamPacketSender,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &crate::runtime::Proxy,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    config: vmess::udp::VmessUdpFlowConfig<'_>,
    transport: crate::transport::VmessTransportOptions<'_>,
    payload: &[u8],
) -> Result<(), EngineError> {
    let stream = crate::transport::build_vmess_outbound_transport_over_stream(
        crate::transport::VmessFinalHopTransportRequest {
            carrier,
            options: transport,
        },
    )
    .await?;
    let upstream = establish::over_stream(proxy, session, config, payload, stream).await?;
    upstreams.insert_and_bridge_target(session.target.clone(), session.port, chain_tasks, upstream);
    Ok(())
}
