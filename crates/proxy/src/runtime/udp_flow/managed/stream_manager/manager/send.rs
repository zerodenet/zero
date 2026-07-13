use zero_engine::EngineError;

use super::super::super::model::ManagedStreamExistingSend;
use super::super::connector::ManagedStreamFlowConnector;
use super::mismatch::managed_mismatch;
use super::model::ManagedStreamFlowManager;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::Proxy;

impl<T> ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    pub(super) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        session: &zero_core::Session,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let connector_flow = resume.connector_flow(endpoint, session_id);
        let (cache_key, requires_relay_upstream) = connector_flow.into_parts();
        if requires_relay_upstream {
            if let Some(sent) = self
                .upstreams
                .send_existing_key(cache_key, ctx.chain_tasks, session_id, packet_ref)
                .await
                .map_err(|error| FlowFailure {
                    stage: self.relay_send_stage,
                    error,
                    upstream: Some(endpoint.upstream()),
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
                upstream: Some(endpoint.upstream()),
            });
        }

        self.upstreams
            .send_or_insert_key(
                cache_key,
                ctx.chain_tasks,
                session_id,
                packet_ref,
                resume.establish_direct(proxy, session, endpoint),
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.establish_stage,
                error,
                upstream: Some(endpoint.upstream()),
            })
    }

    pub(super) async fn send_managed_existing(
        &mut self,
        request: ManagedStreamExistingSend<'_>,
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
            request.session,
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
