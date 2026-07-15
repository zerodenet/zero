use async_trait::async_trait;

use super::super::super::flow::ManagedUdpFlowResume;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use super::super::super::model::ManagedStreamPacketFlowHandler;
use super::super::super::model::{ManagedRelayExistingSend, ManagedRelayFlowHandler};
use super::super::connector::ManagedStreamFlowConnector;
use super::mismatch::managed_mismatch;
use super::model::{
    ManagedStreamFlowManager, ManagedStreamRelayRequest, SharedManagedStreamFlowManager,
};
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;

impl<T> ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    async fn send_relay(
        &mut self,
        request: ManagedStreamRelayRequest<'_, T>,
    ) -> Result<usize, FlowFailure> {
        let ManagedStreamRelayRequest {
            ctx,
            stream,
            tls_server_name,
            services,
            session,
            endpoint,
            resume,
            packet_ref,
        } = request;
        let session_id = ctx.session_id;
        let upstream = endpoint.upstream();
        let (cache_key, _) = resume
            .connector_flow(endpoint.clone(), session_id)
            .into_parts();
        let entry = resume
            .establish_relay(stream, tls_server_name, services, session, endpoint)
            .await
            .map_err(|error| FlowFailure {
                stage: self.relay_establish_stage,
                error,
                upstream: Some(upstream.clone()),
            })?;

        self.upstreams
            .insert_and_send_key(cache_key, ctx.chain_tasks, session_id, packet_ref, entry)
            .await
            .map_err(|error| FlowFailure {
                stage: self.relay_send_stage,
                error,
                upstream: Some(upstream),
            })
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelayExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
        self.send_relay(ManagedStreamRelayRequest {
            ctx: UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            stream: request.stream,
            tls_server_name: request.tls_server_name,
            services: request.services,
            session: request.session,
            endpoint: OutboundEndpoint {
                server: request.server.to_owned(),
                port: request.port,
            },
            resume,
            packet_ref: UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        })
        .await
    }
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
#[async_trait]
impl<T> ManagedStreamPacketFlowHandler for SharedManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send_managed_existing(
        &mut self,
        request: super::super::super::model::ManagedStreamExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.0.lock().await.send_managed_existing(request).await
    }
}

#[async_trait]
impl<T> ManagedRelayFlowHandler for SharedManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelayExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.0
            .lock()
            .await
            .send_managed_relay_existing(request)
            .await
    }
}
