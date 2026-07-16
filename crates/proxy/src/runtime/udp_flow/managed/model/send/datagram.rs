use tokio::task::JoinSet;
use zero_core::Address;

use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::managed::flow::{ManagedDatagramFlow, ManagedUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

pub(crate) struct ManagedDatagramExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

impl<'a> ManagedDatagramExistingSend<'a> {
    pub(crate) fn datagram(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        flow: &ManagedDatagramFlow<'a>,
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            services: flow.services.clone(),
            server: flow.server,
            port: flow.port,
            resume: flow.resume.clone(),
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: flow.payload,
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
            services: Some(services),
            server,
            port,
            resume,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload,
        }
    }
}
