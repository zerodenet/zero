use zero_core::Session;

use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    request: super::ShadowsocksUdpFlowStart<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: Some(proxy),
            tag: request.tag,
            session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload,
        })
        .await
}
