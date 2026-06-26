use crate::protocol_runtime::vmess_udp::model::{VmessUdpFlow, VmessUdpRelayFlow};
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

impl UdpDispatch {
    pub(crate) async fn send_vmess_datagram(
        &mut self,
        request: VmessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vmess_udp_flow(&mut self.chain_tasks, request)
            .await
    }

    pub(crate) async fn send_vmess_relay(
        &mut self,
        request: VmessUdpRelayFlow<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vmess_udp_relay_flow(&mut self.chain_tasks, request)
            .await
    }
}
