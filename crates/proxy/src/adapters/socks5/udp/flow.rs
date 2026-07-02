use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, ManagedRelayStart, UdpDispatch};

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    request: ManagedRelayStart<'_, socks5::udp::Socks5UdpFlowResume>,
) -> Result<FlowStartResult, FlowFailure> {
    dispatch
        .start_tracked_managed_relay(request)
        .await
        .map_err(|failure| FlowFailure {
            stage: failure.stage,
            error: failure.error,
            upstream: failure.upstream,
        })
}
