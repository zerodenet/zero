use tokio::task::JoinSet;
use zero_core::{Address, Session};

use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::managed::flow::{ManagedStreamPacketFlow, ManagedUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

pub(crate) struct ManagedStreamExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) services: UdpRuntimeServices,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

impl<'a> ManagedStreamExistingSend<'a> {
    pub(crate) fn stream_packet(request: ManagedStreamPacketFlow<'a>) -> Self {
        Self {
            chain_tasks: request.chain_tasks,
            session_id: request.session.id,
            services: request.services,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            target: &request.session.target,
            target_port: request.session.port,
            payload: request.payload,
        }
    }

    pub(crate) fn forwarded(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        services: UdpRuntimeServices,
        flow: &'a UdpFlowSnapshot,
        resume: ManagedUdpFlowResume,
        server: &'a str,
        port: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            services,
            session: &flow.session,
            server,
            port,
            resume,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload,
        }
    }
}
