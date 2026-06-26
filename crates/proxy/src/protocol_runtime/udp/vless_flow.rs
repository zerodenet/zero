pub(crate) use crate::protocol_runtime::vless_udp::model::{
    VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream,
};
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

pub(crate) async fn send_datagram(
    dispatch: &mut UdpDispatch,
    request: VlessUdpFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    protocol_state
        .start_vless_udp_flow(chain_tasks, request)
        .await
}

pub(crate) async fn send_relay_two_stream(
    dispatch: &mut UdpDispatch,
    request: VlessUdpRelayTwoStream<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    protocol_state
        .start_vless_udp_relay_two_stream(chain_tasks, request)
        .await
}

pub(crate) async fn send_relay_final_hop(
    dispatch: &mut UdpDispatch,
    request: VlessUdpRelayFinalHop<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    protocol_state
        .start_vless_udp_relay_final_hop(chain_tasks, request)
        .await
}
