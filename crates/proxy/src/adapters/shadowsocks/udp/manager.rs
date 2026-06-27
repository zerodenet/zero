use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedDatagramConnectionCache;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::managed::{ManagedDatagramFlowHandler, ManagedExistingSend};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;

mod bridge;
mod entry;
pub(super) mod model;

use model::SsSendExisting;

pub(crate) struct SsChainManager {
    upstreams: ManagedDatagramConnectionCache,
}

impl SsChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedDatagramConnectionCache::new(),
        }
    }

    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume
            .as_ref::<shadowsocks::ShadowsocksUdpFlowResume>()
            .is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        endpoint: OutboundEndpoint<'_>,
        resume: shadowsocks::ShadowsocksUdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let target_addr = proxy
            .protocols
            .direct_connector()
            .resolve_address(
                &endpoint.address(),
                endpoint.port,
                proxy.resolver.as_ref(),
                "failed to resolve shadowsocks udp upstream",
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "ss_resolve_addr",
                error: error.into(),
                upstream: Some(endpoint.upstream()),
            })?;

        let cache_key = resume.flow_cache_key();
        let entry = entry::ensure(&mut self.upstreams, cache_key, &resume, target_addr)
            .await
            .map_err(|error| FlowFailure {
                stage: "ss_establish",
                error,
                upstream: Some(endpoint.upstream()),
            })?;

        entry
            .send_datagram(
                ctx.chain_tasks,
                ctx.session_id,
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "ss_send",
                error: e,
                upstream: Some(endpoint.upstream()),
            })
    }

    async fn send_existing(&mut self, request: SsSendExisting<'_>) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            request.resume,
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
        let Some(resume) = request
            .resume
            .cloned::<shadowsocks::ShadowsocksUdpFlowResume>()
        else {
            return Err(managed_mismatch(
                "udp_shadowsocks_resume",
                request.server,
                request.port,
                "expected Shadowsocks UDP flow resume",
            ));
        };
        let Some(proxy) = request.proxy else {
            return Err(managed_mismatch(
                "udp_shadowsocks_proxy",
                request.server,
                request.port,
                "expected proxy context for Shadowsocks UDP flow",
            ));
        };
        self.send_existing(SsSendExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            proxy,
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
impl ManagedDatagramFlowHandler for SsChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        SsChainManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        SsChainManager::send_managed_existing(self, request).await
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
