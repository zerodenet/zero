use std::any::Any;

use super::request::ManagedStreamPacketStartBridge;
use crate::runtime::udp_flow::managed::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};
use crate::runtime::udp_flow::state::UdpFlowStartContext;

async fn start_managed_stream_packet<T>(
    context: &mut UdpFlowStartContext<'_>,
    request: ManagedStreamPacketStartBridge<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    let resume = ManagedUdpFlowResume::new(request.resume);
    let tag = request.tag.to_owned();
    let server = request.server.to_owned();
    let port = request.port;
    let relay_chain = request.relay_chain;
    let sent = context
        .start_managed_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            services: request.services,
            kind: if relay_chain {
                ManagedUdpFlowKind::RelayStream
            } else {
                ManagedUdpFlowKind::StreamPacket
            },
            session: request.session,
            carrier: request.carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: resume.clone(),
            payload: request.payload,
        })
        .await?;
    let managed = context.register_managed_flow(resume);
    let outbound = if relay_chain {
        UdpFlowOutbound::Relay {
            tag,
            server,
            port,
            managed,
        }
    } else {
        UdpFlowOutbound::StreamPacket {
            tag,
            server,
            port,
            managed,
        }
    };
    Ok(FlowStartResult::Flow {
        outbound: Box::new(outbound),
        tx_bytes: sent as u64,
    })
}

pub(crate) async fn start_direct_managed_stream_packet<T>(
    context: &mut UdpFlowStartContext<'_>,
    request: ManagedStreamPacketStartBridge<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_managed_stream_packet(context, request).await
}

pub(crate) async fn start_relay_managed_stream_packet<T>(
    context: &mut UdpFlowStartContext<'_>,
    request: ManagedStreamPacketStartBridge<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_managed_stream_packet(context, request).await
}
