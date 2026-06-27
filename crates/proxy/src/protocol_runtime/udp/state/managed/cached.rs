use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use crate::protocol_runtime::vless_udp::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow,
};
use crate::protocol_runtime::vless_udp::VlessCachedFlowHandler;
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::VmessCachedFlowHandler;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(crate) struct ManagedCachedHandlers {
    pub(crate) vless: Box<dyn VlessCachedFlowHandler>,
    #[cfg(feature = "vmess")]
    pub(crate) vmess: Box<dyn VmessCachedFlowHandler>,
}

pub(in crate::protocol_runtime::udp::state::managed) struct ManagedCachedState {
    handlers: ManagedCachedHandlers,
}

impl ManagedCachedState {
    pub(in crate::protocol_runtime::udp::state::managed) fn new(
        handlers: ManagedCachedHandlers,
    ) -> Self {
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        if let Some(session_id) = self
            .handlers
            .vless
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }
        #[cfg(feature = "vmess")]
        if let Some(session_id) = self
            .handlers
            .vmess
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }

        Ok(None)
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_vless_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpStartFlow<'_>,
    ) -> Option<Result<(), EngineError>> {
        Some(
            self.handlers
                .vless
                .start_vless_flow(chain_tasks, flow)
                .await,
        )
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_vless_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Option<Result<(), EngineError>> {
        Some(
            self.handlers
                .vless
                .start_vless_relay_two_stream(chain_tasks, flow)
                .await,
        )
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_vless_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayFinalHopStart<'_>,
    ) -> Option<Result<(), EngineError>> {
        Some(
            self.handlers
                .vless
                .start_vless_relay_final_hop(chain_tasks, flow)
                .await,
        )
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp::state::managed) async fn start_vmess_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpStartFlow<'_>,
    ) -> Option<Result<(), EngineError>> {
        Some(
            self.handlers
                .vmess
                .start_vmess_flow(chain_tasks, flow)
                .await,
        )
    }

    #[cfg(feature = "vmess")]
    pub(in crate::protocol_runtime::udp::state::managed) async fn start_vmess_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpRelayFlowStart<'_>,
    ) -> Option<Result<(), EngineError>> {
        Some(
            self.handlers
                .vmess
                .start_vmess_relay_flow(chain_tasks, flow)
                .await,
        )
    }
}
