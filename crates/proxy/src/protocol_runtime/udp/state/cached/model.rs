use crate::protocol_runtime::udp::state::managed::model::ManagedCachedFlowSender;
use crate::protocol_runtime::vless_udp::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow,
};
#[cfg(feature = "vmess")]
use crate::protocol_runtime::vmess_udp::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;
use zero_engine::EngineError;

pub(crate) enum CachedUdpFlowStart<'a> {
    Vless(VlessUdpStartFlow<'a>),
    VlessRelayTwoStream(VlessUdpRelayTwoStream<'a>),
    VlessRelayFinalHop(VlessUdpRelayFinalHopStart<'a>),
    #[cfg(feature = "vmess")]
    Vmess(VmessUdpStartFlow<'a>),
    #[cfg(feature = "vmess")]
    VmessRelay(VmessUdpRelayFlowStart<'a>),
}

#[async_trait::async_trait]
pub(crate) trait CachedUdpFlowHandler: ManagedCachedFlowSender {
    async fn try_start_cached_flow<'a>(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        request: CachedUdpFlowStart<'a>,
    ) -> Result<Option<CachedUdpFlowStart<'a>>, EngineError>;
}

pub(crate) struct CachedUdpHandlers {
    pub(crate) cached: Vec<Box<dyn CachedUdpFlowHandler>>,
}

pub(in crate::protocol_runtime::udp::state) struct CachedProtocolUdpState {
    handlers: CachedUdpHandlers,
}

impl CachedProtocolUdpState {
    pub(in crate::protocol_runtime::udp::state) fn new(handlers: CachedUdpHandlers) -> Self {
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state) fn senders(
        &mut self,
    ) -> impl Iterator<Item = &mut dyn ManagedCachedFlowSender> {
        self.handlers
            .cached
            .iter_mut()
            .map(|handler| handler.as_mut() as &mut dyn ManagedCachedFlowSender)
    }

    pub(in crate::protocol_runtime::udp::state) async fn start_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        mut request: CachedUdpFlowStart<'_>,
    ) -> Result<(), EngineError> {
        for handler in &mut self.handlers.cached {
            match handler.try_start_cached_flow(chain_tasks, request).await? {
                Some(unhandled) => request = unhandled,
                None => return Ok(()),
            }
        }
        Err(EngineError::Io(std::io::Error::other(
            "cached UDP flow has no compiled handler",
        )))
    }
}
