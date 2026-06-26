use super::super::state::ProtocolUdpState;
use super::super::{FlowFailure, ManagedUdpFlowRequest};

impl ProtocolUdpState {
    pub(crate) async fn start_socks5_relay_flow(
        &mut self,
        inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.socks5.start_relay_flow(inbound_tag, request).await
    }
}
