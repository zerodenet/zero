use zero_core::Session;
use zero_transport::hysteria2_quic::Hysteria2ManagedUdpFlowPlan;

use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    session: &Session,
    payload: &[u8],
    plan: Hysteria2ManagedUdpFlowPlan<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let (tag, server, port, resume) = plan.into_parts();
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: None::<&Proxy>,
            tag,
            session,
            server,
            port,
            resume,
            payload,
        })
        .await
}
