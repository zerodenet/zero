use super::error::managed_forward_unavailable;
use super::model::ManagedUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

impl ManagedUdpState {
    pub(crate) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
        is_upstream_resume: impl FnOnce(&ManagedUdpFlowResume) -> bool,
    ) -> Result<Option<usize>, FlowFailure> {
        let Some(flow_ref) = flow.outbound.managed_flow() else {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "direct, relay, and packet-path flows are handled outside managed UDP state",
            ));
        };

        let Some(resume) = self.flow_resume(flow_ref) else {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "managed UDP flow resume was dropped",
            ));
        };

        if is_upstream_resume(&resume) {
            return Err(managed_forward_unavailable(
                "udp_protocol_forward",
                "upstream association flows are handled by generic UDP dispatch",
            ));
        }

        if let Some(result) = self
            .datagram
            .forward_existing_flow(chain_tasks, proxy, flow, &resume, payload)
            .await
        {
            return result.map(Some);
        }
        if let Some(result) = self
            .stream
            .forward_existing_flow(chain_tasks, proxy, flow, &resume, payload)
            .await
        {
            return result.map(Some);
        }

        Ok(None)
    }
}
