use super::model::ManagedUdpState;
use crate::runtime::udp_flow::managed::flow::{ManagedExistingFlowForward, ManagedUdpFlowResume};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

impl ManagedUdpState {
    pub(crate) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        request: ManagedExistingFlowForward<'_>,
        resume: ManagedUdpFlowResume,
    ) -> Result<Option<usize>, FlowFailure> {
        let (flow, payload) = request;
        #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
        if let Some(result) = self
            .datagram
            .forward_existing_flow(chain_tasks, proxy, flow, &resume, payload)
            .await
        {
            return result.map(Some);
        }
        #[cfg(any(
            feature = "vless",
            feature = "vmess",
            feature = "trojan",
            feature = "mieru"
        ))]
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
