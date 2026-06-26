#[cfg(feature = "mieru")]
use crate::protocol_runtime::udp::mieru_manager::MieruChainManager;
use crate::protocol_runtime::udp::state::managed::flow_mismatch;
use crate::protocol_runtime::udp::state::managed::model::{
    ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler,
};
#[cfg(feature = "trojan")]
use crate::protocol_runtime::udp::trojan_manager::TrojanChainManager;
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

use crate::protocol_runtime::udp::flows::{ManagedRelayStreamFlow, ManagedStreamPacketFlow};

pub(in crate::protocol_runtime::udp::state::managed) struct ManagedStreamState {
    handlers: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

impl ManagedStreamState {
    pub(in crate::protocol_runtime::udp::state::managed) fn new() -> Self {
        let handlers: Vec<Box<dyn ManagedStreamFlowHandler>> = vec![
            #[cfg(feature = "trojan")]
            (Box::new(TrojanChainManager::new()) as Box<dyn ManagedStreamFlowHandler>),
            #[cfg(feature = "mieru")]
            (Box::new(MieruChainManager::new()) as Box<dyn ManagedStreamFlowHandler>),
        ];
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        for handler in &mut self.handlers {
            if !handler.supports_managed_existing(&request.resume) {
                continue;
            }
            return handler
                .send_managed_existing(ManagedExistingSend::stream_packet(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_stream_packet_resume",
            request.server,
            request.port,
            "expected stream-packet UDP flow resume",
        ))
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        for handler in &mut self.handlers {
            if !handler.supports_managed_relay_existing(&request.resume) {
                continue;
            }
            return handler
                .send_managed_relay_existing(ManagedRelaySend::relay_stream(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_relay_stream_resume",
            request.server,
            request.port,
            "expected relay-stream UDP flow resume",
        ))
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ProtocolUdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
        let resume = snapshot.resume();
        for handler in &mut self.handlers {
            if !handler.supports_managed_existing(resume) {
                continue;
            }
            return Some(
                handler
                    .send_managed_existing(ManagedExistingSend::forwarded(
                        chain_tasks,
                        proxy,
                        flow,
                        resume.clone(),
                        upstream.server,
                        upstream.port,
                        payload,
                    ))
                    .await,
            );
        }
        None
    }
}
