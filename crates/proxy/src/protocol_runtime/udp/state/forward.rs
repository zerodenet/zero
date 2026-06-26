use tokio::task::JoinSet;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, ProtocolUdpFlowResume};
#[cfg(feature = "hysteria2")]
mod hysteria2;
#[cfg(feature = "mieru")]
mod mieru;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "trojan")]
mod trojan;
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
        let Some(snapshot) = flow.outbound.protocol_snapshot() else {
            return Err(protocol_forward_unavailable(
                "udp_protocol_forward",
                "direct, relay, and packet-path flows are handled outside protocol snapshots",
            ));
        };

        if matches!(snapshot.resume(), ProtocolUdpFlowResume::Socks5(_)) {
            return Err(protocol_forward_unavailable(
                "udp_protocol_forward",
                "SOCKS5 relay flows are handled by generic UDP dispatch",
            ));
        }

        #[cfg(feature = "shadowsocks")]
        if let Some(result) =
            shadowsocks::forward_if_matches(self, chain_tasks, proxy, flow, snapshot, payload).await
        {
            return result;
        }
        #[cfg(feature = "hysteria2")]
        if let Some(result) =
            hysteria2::forward_if_matches(self, chain_tasks, flow, snapshot, payload).await
        {
            return result;
        }
        #[cfg(feature = "trojan")]
        if let Some(result) =
            trojan::forward_if_matches(self, chain_tasks, proxy, flow, snapshot, payload).await
        {
            return result;
        }
        #[cfg(feature = "mieru")]
        if let Some(result) =
            mieru::forward_if_matches(self, chain_tasks, proxy, flow, snapshot, payload).await
        {
            return result;
        }

        Err(protocol_forward_unavailable(
            "udp_protocol_forward",
            "protocol UDP flow snapshot has no compiled forward handler",
        ))
    }
}
