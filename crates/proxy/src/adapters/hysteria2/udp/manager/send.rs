use super::model::H2SendExisting;
use super::{establish, H2ChainManager};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::managed::{
    ManagedDatagramFlowHandler, ManagedExistingSend, ManagedUdpConnectionCacheKey,
};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};

impl H2ChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume
            .as_ref::<hysteria2::Hysteria2UdpFlowResume>()
            .is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        endpoint: OutboundEndpoint<'_>,
        resume: hysteria2::Hysteria2UdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let cache_key = ManagedUdpConnectionCacheKey::new(
            resume.flow_cache_key(endpoint.server, endpoint.port),
        );

        self.upstreams
            .send_or_insert_pre_sent(
                cache_key,
                ctx.chain_tasks,
                ctx.session_id,
                packet_ref,
                establish::upstream(endpoint, resume.clone(), packet_ref),
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "h2_establish",
                error: e,
                upstream: Some(endpoint.upstream()),
            })
    }

    async fn send_existing(&mut self, request: H2SendExisting<'_>) -> Result<usize, FlowFailure> {
        let resume = request.resume.clone();
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<hysteria2::Hysteria2UdpFlowResume>() else {
            return Err(managed_mismatch(
                "udp_hysteria2_resume",
                request.server,
                request.port,
                "expected Hysteria2 UDP flow resume",
            ));
        };
        self.send_existing(H2SendExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            server: request.server,
            port: request.port,
            resume,
            target: request.target,
            target_port: request.target_port,
            payload: request.payload,
        })
        .await
    }
}

#[async_trait::async_trait]
impl ManagedDatagramFlowHandler for H2ChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        H2ChainManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        H2ChainManager::send_managed_existing(self, request).await
    }
}

fn managed_mismatch(
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
