use crate::protocol_runtime::udp::{VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream};
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

impl UdpDispatch {
    pub(crate) async fn send_vless_datagram(
        &mut self,
        request: VlessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_flow(&mut self.chain_tasks, request)
            .await
    }

    pub(crate) async fn send_vless_relay_two_stream(
        &mut self,
        request: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_relay_two_stream(&mut self.chain_tasks, request)
            .await
    }

    pub(crate) async fn send_vless_relay_final_hop(
        &mut self,
        request: VlessUdpRelayFinalHop<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_relay_final_hop(&mut self.chain_tasks, request)
            .await
    }
}
