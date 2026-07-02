use zero_core::Session;

use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    session: &Session,
    request: super::Hysteria2UdpFlowStart<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: None,
            tag: request.tag,
            session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload,
        })
        .await
}
