use tokio::task::JoinSet;

use super::super::state::ProtocolUdpState;
use super::super::{FlowFailure, ManagedDatagramFlow};
use crate::runtime::udp_flow::packet_path::ChainTask;

impl ProtocolUdpState {
    pub(crate) async fn start_managed_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.managed.start_datagram_flow(chain_tasks, flow).await
    }
}
