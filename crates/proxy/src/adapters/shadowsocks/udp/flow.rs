use zero_core::Session;
use zero_transport::shadowsocks_transport::ShadowsocksManagedUdpFlowPlan;

use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    plan: ShadowsocksManagedUdpFlowPlan<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let (tag, server, port, resume) = plan.into_parts();
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: Some(proxy),
            tag,
            session,
            server,
            port,
            resume,
            payload,
        })
        .await
}
