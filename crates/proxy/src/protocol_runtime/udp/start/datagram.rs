use tokio::task::JoinSet;
use zero_core::Session;

#[cfg(feature = "hysteria2")]
use super::super::h2_manager::model::H2SendExisting;
#[cfg(feature = "shadowsocks")]
use super::super::ss_manager::model::SsSendExisting;
use super::super::state::ProtocolUdpState;
#[cfg(feature = "shadowsocks")]
use super::super::ShadowsocksUdpFlow;
use super::super::{ChainTask, FlowFailure, ProtocolUdpFlowResume};

impl ProtocolUdpState {
    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn start_shadowsocks_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ShadowsocksUdpFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Shadowsocks(resume) = &flow.resume else {
            return Err(resume_mismatch(
                "udp_shadowsocks_resume",
                flow.server,
                flow.port,
                "expected Shadowsocks UDP flow resume",
            ));
        };
        self.shadowsocks
            .send_existing(SsSendExisting {
                chain_tasks,
                session_id: flow.session.id,
                proxy: flow.proxy,
                server: flow.server,
                port: flow.port,
                cache_key: resume.cache_key().to_owned(),
                codec: std::sync::Arc::new(resume.codec()),
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: flow.payload,
            })
            .await
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn start_hysteria2_udp_flow(
        &mut self,
        request: Hysteria2UdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Hysteria2(resume) = &request.resume else {
            return Err(resume_mismatch(
                "udp_hysteria2_resume",
                request.server,
                request.port,
                "expected Hysteria2 UDP flow resume",
            ));
        };
        self.hysteria2
            .send_existing(H2SendExisting {
                chain_tasks: request.chain_tasks,
                session_id: request.session.id,
                server: request.server,
                port: request.port,
                password: resume.password(),
                client_fingerprint: resume.client_fingerprint(),
                codec: std::sync::Arc::new(resume.codec()),
                target: &request.session.target,
                target_port: request.session.port,
                payload: request.payload,
            })
            .await
    }
}

#[cfg(feature = "hysteria2")]
pub(crate) struct Hysteria2UdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
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
