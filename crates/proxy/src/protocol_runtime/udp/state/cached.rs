use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::protocol_runtime::vless_udp::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow,
};
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(in crate::protocol_runtime::udp::state) mod model;

pub(in crate::protocol_runtime::udp::state) use model::CachedProtocolUdpState;
pub(crate) use model::CachedUdpHandlers;

impl ProtocolUdpState {
    pub(crate) async fn send_existing_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        for sender in self.cached.senders() {
            if let Some(session_id) = sender
                .send_existing(chain_tasks, proxy, target, port, payload)
                .await?
            {
                return Ok(Some(session_id));
            }
        }

        Ok(None)
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .vless()
            .start_vless_flow(chain_tasks, flow)
            .await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_cached_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .vless()
            .start_vless_relay_two_stream(chain_tasks, flow)
            .await
    }

    pub(in crate::protocol_runtime::udp) async fn start_vless_cached_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayFinalHopStart<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .vless()
            .start_vless_relay_final_hop(chain_tasks, flow)
            .await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .vmess()
            .start_vmess_flow(chain_tasks, flow)
            .await
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp) async fn start_vmess_cached_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        self.cached
            .vmess()
            .start_vmess_relay_flow(chain_tasks, flow)
            .await
    }
}
