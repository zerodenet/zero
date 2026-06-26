use tokio::task::JoinSet;

#[cfg(feature = "hysteria2")]
use super::super::h2_manager::model::H2SendExisting;
#[cfg(feature = "shadowsocks")]
use super::super::ss_manager::model::SsSendExisting;
use super::super::state::ProtocolUdpState;
use super::super::{FlowFailure, ManagedDatagramFlow, ProtocolUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;

impl ProtocolUdpState {
    pub(crate) async fn start_managed_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        match &flow.resume {
            #[cfg(feature = "shadowsocks")]
            ProtocolUdpFlowResume::Shadowsocks(resume) => {
                let Some(proxy) = flow.proxy else {
                    return Err(resume_mismatch(
                        "udp_shadowsocks_proxy",
                        flow.server,
                        flow.port,
                        "expected proxy context for Shadowsocks UDP flow",
                    ));
                };
                self.shadowsocks
                    .send_existing(SsSendExisting {
                        chain_tasks,
                        session_id: flow.session.id,
                        proxy,
                        server: flow.server,
                        port: flow.port,
                        resume: resume.clone(),
                        target: &flow.session.target,
                        target_port: flow.session.port,
                        payload: flow.payload,
                    })
                    .await
            }
            #[cfg(feature = "hysteria2")]
            ProtocolUdpFlowResume::Hysteria2(resume) => {
                self.hysteria2
                    .send_existing(H2SendExisting {
                        chain_tasks,
                        session_id: flow.session.id,
                        server: flow.server,
                        port: flow.port,
                        resume: resume.clone(),
                        target: &flow.session.target,
                        target_port: flow.session.port,
                        payload: flow.payload,
                    })
                    .await
            }
            _ => Err(resume_mismatch(
                "udp_managed_datagram_resume",
                flow.server,
                flow.port,
                "expected managed datagram UDP flow resume",
            )),
        }
    }
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
