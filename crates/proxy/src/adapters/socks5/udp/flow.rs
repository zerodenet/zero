use zero_core::Session;
use zero_transport::socks5_transport::Socks5ManagedUdpFlowPlan;

use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, UdpDispatch, UpstreamTrackedStart,
};
use crate::runtime::udp_flow::registered::{
    boxed_registered_upstream_handler, UpstreamAssociationHandler, UpstreamAssociationStages,
    UpstreamAssociationTarget,
};
use crate::runtime::Proxy;

impl UpstreamAssociationTarget
    for zero_transport::socks5_transport::Socks5ManagedUdpAssociationTarget
{
    fn outbound_tag(&self) -> &str {
        self.outbound_tag()
    }

    fn log_parts(&self) -> (&str, &str, u16) {
        self.log_parts()
    }
}

pub(super) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    boxed_registered_upstream_handler::<
        zero_transport::socks5_transport::Socks5ManagedUdpAssociationTarget,
        zero_transport::socks5_transport::Socks5UpstreamUdpAssociation,
    >(UpstreamAssociationStages::new(
        "udp_socks5_proxy",
        "udp_socks5_resume",
        "expected SOCKS5 UDP association target",
    ))
}

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    plan: Socks5ManagedUdpFlowPlan<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let (tag, server, port, association_target) = plan.into_parts();
    dispatch
        .start_tracked_upstream(UpstreamTrackedStart {
            proxy: Some(proxy),
            tag,
            session,
            server,
            port,
            resume: association_target,
            payload,
        })
        .await
        .map_err(|failure| FlowFailure {
            stage: failure.stage,
            error: failure.error,
            upstream: failure.upstream,
        })
}
