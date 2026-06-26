use tokio::task::JoinSet;
use zero_core::Session;

use super::super::mieru_manager::model::{MieruRelayExisting, MieruSendExisting};
use super::super::state::ProtocolUdpState;
use super::super::{ChainTask, FlowFailure, MieruUdpRelayFlow, ProtocolUdpFlowResume};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) async fn start_mieru_udp_flow(
        &mut self,
        request: MieruUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Mieru(resume) = &request.resume else {
            return Err(resume_mismatch(
                "udp_mieru_resume",
                request.server,
                request.port,
                "expected Mieru UDP flow resume",
            ));
        };
        self.mieru
            .send_existing(MieruSendExisting {
                chain_tasks: request.chain_tasks,
                session_id: request.session.id,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                username: resume.username(),
                password: resume.password(),
                relay_chain: resume.relay_chain(),
                target: &request.session.target,
                target_port: request.session.port,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn start_mieru_udp_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: MieruUdpRelayFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Mieru(resume) = &flow.resume else {
            return Err(resume_mismatch(
                "udp_mieru_resume",
                flow.server,
                flow.port,
                "expected Mieru UDP flow resume",
            ));
        };
        self.mieru
            .send_relay_existing(MieruRelayExisting {
                chain_tasks,
                session_id: flow.session.id,
                stream: flow.carrier.stream,
                server: flow.server,
                port: flow.port,
                username: resume.username(),
                password: resume.password(),
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: flow.payload,
            })
            .await
    }
}

pub(crate) struct MieruUdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub resume: ProtocolUdpFlowResume,
    pub payload: &'a [u8],
}

fn resume_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
