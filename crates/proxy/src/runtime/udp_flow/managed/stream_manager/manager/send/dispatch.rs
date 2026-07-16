use zero_engine::EngineError;

use super::super::super::connector::ManagedStreamFlowConnector;
use super::super::model::ManagedStreamFlowManager;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;

impl<T> ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    pub(super) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        services: UdpRuntimeServices,
        session: &zero_core::Session,
        endpoint: OutboundEndpoint,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let upstream = endpoint.upstream();
        let connector_flow = resume.connector_flow(endpoint.clone(), session_id);
        let (cache_key, requires_relay_upstream) = connector_flow.into_parts();
        if requires_relay_upstream {
            if let Some(sent) = self
                .upstreams
                .send_existing_key(cache_key, ctx.chain_tasks, session_id, packet_ref)
                .await
                .map_err(|error| FlowFailure {
                    stage: self.relay_send_stage,
                    error,
                    upstream: Some(upstream.clone()),
                })?
            {
                return Ok(sent);
            }
            return Err(FlowFailure {
                stage: self.relay_upstream_stage,
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "relay upstream is not established",
                )),
                upstream: Some(upstream.clone()),
            });
        }

        self.upstreams
            .send_or_insert_key(
                cache_key,
                ctx.chain_tasks,
                session_id,
                packet_ref,
                resume.establish_direct(services, session, endpoint),
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.establish_stage,
                error,
                upstream: Some(upstream),
            })
    }
}
