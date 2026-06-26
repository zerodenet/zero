use tokio::task::JoinSet;
use zero_core::Session;

use super::super::state::ProtocolUdpState;
use super::super::trojan_manager::model::{TrojanRelayExisting, TrojanSendExisting};
use super::super::{ChainTask, FlowFailure, ProtocolUdpFlowResume};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) async fn start_trojan_udp_flow(
        &mut self,
        request: TrojanUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Trojan(resume) = &request.resume else {
            return Err(resume_mismatch(
                "udp_trojan_resume",
                request.server,
                request.port,
                "expected Trojan UDP flow resume",
            ));
        };
        self.trojan
            .send_existing(TrojanSendExisting {
                chain_tasks: request.chain_tasks,
                session_id: request.session.id,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                password: resume.password(),
                sni: resume.sni(),
                insecure: resume.insecure(),
                client_fingerprint: resume.client_fingerprint(),
                relay_chain: resume.relay_chain(),
                target: &request.session.target,
                target_port: request.session.port,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn start_trojan_udp_relay_flow(
        &mut self,
        request: TrojanUdpRelayFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Trojan(resume) = &request.resume else {
            return Err(resume_mismatch(
                "udp_trojan_resume",
                request.server,
                request.port,
                "expected Trojan UDP flow resume",
            ));
        };
        self.trojan
            .send_relay_existing(TrojanRelayExisting {
                chain_tasks: request.chain_tasks,
                session_id: request.session.id,
                stream: request.carrier.stream,
                tls_server_name: None,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                password: resume.password(),
                sni: resume.sni(),
                insecure: resume.insecure(),
                client_fingerprint: resume.client_fingerprint(),
                target: &request.session.target,
                target_port: request.session.port,
                payload: request.payload,
            })
            .await
    }
}

pub(crate) struct TrojanUdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub resume: ProtocolUdpFlowResume,
    pub payload: &'a [u8],
}

pub(crate) struct TrojanUdpRelayFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub carrier: crate::transport::RelayCarrier,
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
