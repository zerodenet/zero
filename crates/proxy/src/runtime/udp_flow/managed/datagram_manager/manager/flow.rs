use std::any::Any;

use async_trait::async_trait;

use super::super::super::flow::ManagedUdpFlowResume;
use super::super::super::model::{ManagedDatagramFlowHandler, ManagedExistingSend};
use super::super::connector::ManagedDatagramFlowConnector;
use super::mismatch::managed_mismatch;
use super::model::ManagedDatagramFlowManager;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;

impl<T, C> ManagedDatagramFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: Option<&Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let cache_key = self
            .connector
            .connector_flow(&resume, endpoint)
            .into_cache_key();
        let establish = self
            .connector
            .establish(proxy, endpoint, resume, packet_ref);
        let result = if C::INITIAL_PACKET_PRE_SENT {
            self.upstreams
                .send_or_insert_pre_sent_key(
                    cache_key,
                    ctx.chain_tasks,
                    ctx.session_id,
                    packet_ref,
                    establish,
                )
                .await
        } else {
            self.upstreams
                .send_or_insert_key(
                    cache_key,
                    ctx.chain_tasks,
                    ctx.session_id,
                    packet_ref,
                    establish,
                )
                .await
        };
        result.map_err(|error| FlowFailure {
            stage: self.establish_stage,
            error,
            upstream: Some(endpoint.upstream()),
        })
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
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
            resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}

#[async_trait]
impl<T, C> ManagedDatagramFlowHandler for ManagedDatagramFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedDatagramFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedDatagramFlowManager::send_managed_existing(self, request).await
    }
}
