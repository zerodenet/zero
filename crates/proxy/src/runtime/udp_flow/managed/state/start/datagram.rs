use super::super::error::flow_mismatch;
use super::super::model::ManagedUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::ManagedDatagramFlow;
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;

impl ManagedUdpState {
    pub(crate) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        let server = flow.server;
        let port = flow.port;
        self.datagram
            .start_datagram_flow(chain_tasks, flow)
            .await
            .ok_or_else(|| {
                flow_mismatch(
                    "udp_managed_datagram_resume",
                    server,
                    port,
                    "expected managed datagram UDP flow resume",
                )
            })?
    }
}
