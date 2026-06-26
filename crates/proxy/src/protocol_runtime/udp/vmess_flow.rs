pub(crate) use crate::protocol_runtime::vmess_udp::model::{VmessUdpFlow, VmessUdpRelayFlow};
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

pub(crate) async fn send_datagram(
    dispatch: &mut UdpDispatch,
    request: VmessUdpFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    protocol_state
        .start_vmess_udp_flow(chain_tasks, request)
        .await
}

pub(crate) async fn send_relay(
    dispatch: &mut UdpDispatch,
    request: VmessUdpRelayFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    protocol_state
        .start_vmess_udp_relay_flow(chain_tasks, request)
        .await
}
