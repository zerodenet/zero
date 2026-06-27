use tokio::task::JoinSet;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

fn protocol_forward_unavailable(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: None,
    }
}

impl ProtocolUdpState {
    pub(crate) async fn forward_existing_protocol_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        let Some(managed) = flow.outbound.managed_flow() else {
            return Err(protocol_forward_unavailable(
                "udp_protocol_forward",
                "direct, relay, and packet-path flows are handled outside protocol snapshots",
            ));
        };

        if let Some(result) = self
            .managed
            .forward_registered_stream_sender(chain_tasks, proxy, managed, flow, payload)
            .await
        {
            return result;
        }

        let Some(snapshot) = self.managed_flow_snapshot(managed) else {
            return Err(protocol_forward_unavailable(
                "udp_protocol_forward",
                "managed UDP flow snapshot was dropped",
            ));
        };

        if self.upstream.handles_resume(snapshot.resume()) {
            return Err(protocol_forward_unavailable(
                "udp_protocol_forward",
                "upstream association flows are handled by generic UDP dispatch",
            ));
        }

        if let Some(result) = self
            .managed
            .forward_existing_flow(chain_tasks, proxy, flow, &snapshot, payload)
            .await
        {
            return result;
        }

        Err(protocol_forward_unavailable(
            "udp_protocol_forward",
            "protocol UDP flow snapshot has no compiled forward handler",
        ))
    }
}
