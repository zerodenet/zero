use super::super::super::contract::{UpstreamAssociationTarget, UpstreamAssociationTransport};
use super::super::association::UpstreamAssociationRuntime;
use super::super::upstream_flow_mismatch;
use crate::runtime::udp_flow::registered::upstream::UpstreamAssociationSend;
use crate::runtime::udp_flow::result::FlowFailure;

pub(crate) async fn start_registered_upstream_flow<T, A>(
    runtime: &mut UpstreamAssociationRuntime<T, A>,
    inbound_tag: &str,
    request: UpstreamAssociationSend<'_>,
    proxy_stage: &'static str,
    resume_stage: &'static str,
    resume_message: &'static str,
) -> Result<usize, FlowFailure>
where
    T: UpstreamAssociationTarget + 'static,
    A: UpstreamAssociationTransport<T>,
{
    let Some(services) = request.services else {
        return Err(upstream_flow_mismatch(
            proxy_stage,
            request.server,
            request.port,
            "expected proxy context for registered upstream UDP flow",
        ));
    };
    let Some(association) = request.resume.cloned::<T>() else {
        return Err(upstream_flow_mismatch(
            resume_stage,
            request.server,
            request.port,
            resume_message,
        ));
    };

    runtime
        .send_packet(
            &services,
            inbound_tag,
            association,
            request.session,
            request.payload,
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_upstream_send",
            error,
            upstream: Some((request.server.to_string(), request.port)),
        })
}
