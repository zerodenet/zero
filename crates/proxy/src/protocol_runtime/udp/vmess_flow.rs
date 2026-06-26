pub(crate) use crate::protocol_runtime::vmess_udp::model::{VmessUdpFlow, VmessUdpRelayFlow};
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

impl UdpDispatch {
    pub(crate) async fn send_vmess_datagram(
        &mut self,
        request: VmessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let (protocol_state, chain_tasks) = self.protocol_udp_state_and_chain_tasks();
        protocol_state
            .start_vmess_udp_flow(chain_tasks, request)
            .await
    }

    pub(crate) async fn send_vmess_relay(
        &mut self,
        request: VmessUdpRelayFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let (protocol_state, chain_tasks) = self.protocol_udp_state_and_chain_tasks();
        protocol_state
            .start_vmess_udp_relay_flow(chain_tasks, request)
            .await
    }
}
