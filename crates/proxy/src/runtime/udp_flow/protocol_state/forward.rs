use super::ProtocolUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

impl ProtocolUdpState {
    pub(crate) async fn forward_existing_protocol_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        if let Some(result) = self
            .managed
            .forward_existing_flow(chain_tasks, proxy, flow, payload, |resume| {
                self.upstream.handles_resume(resume)
            })
            .await?
        {
            return Ok(result);
        }

        Err(FlowFailure {
            stage: "udp_protocol_forward",
            error: zero_engine::EngineError::Io(std::io::Error::other(
                "protocol UDP flow snapshot has no compiled forward handler",
            )),
            upstream: None,
        })
    }
}
