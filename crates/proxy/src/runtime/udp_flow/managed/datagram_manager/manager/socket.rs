use std::any::Any;

use async_trait::async_trait;

use super::super::super::flow::ManagedUdpFlowResume;
use super::super::super::model::{ManagedDatagramExistingSend, ManagedDatagramFlowHandler};
use super::super::connector::ManagedDatagramSocketFlowConnector;
use super::mismatch::managed_mismatch;
use super::model::ManagedDatagramSocketFlowManager;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;

impl<T, C> ManagedDatagramSocketFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramSocketFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        services: Option<UdpRuntimeServices>,
        endpoint: OutboundEndpoint,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let upstream = endpoint.upstream();
        let cache_key = self
            .connector
            .connector_flow(&resume, endpoint.clone())
            .into_cache_key();
        let connection = self
            .upstreams
            .get_or_insert_key(
                cache_key,
                self.connector
                    .establish(services, endpoint, resume, packet_ref),
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.establish_stage,
                error,
                upstream: Some(upstream.clone()),
            })?;

        connection
            .send_datagram(
                ctx.chain_tasks,
                ctx.session_id,
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.send_stage,
                error,
                upstream: Some(upstream),
            })
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedDatagramExistingSend<'_>,
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
            request.services,
            OutboundEndpoint {
                server: request.server.to_owned(),
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
impl<T, C> ManagedDatagramFlowHandler for ManagedDatagramSocketFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramSocketFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedDatagramSocketFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedDatagramExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedDatagramSocketFlowManager::send_managed_existing(self, request).await
    }
}
