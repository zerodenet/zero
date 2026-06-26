#[cfg(feature = "hysteria2")]
use crate::protocol_runtime::udp::h2_manager::H2ChainManager;
#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::ss_manager::SsChainManager;
use crate::protocol_runtime::udp::state::managed::model::{
    ManagedDatagramFlowHandler, ManagedExistingSend,
};
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

use crate::protocol_runtime::udp::flows::ManagedDatagramFlow;

pub(in crate::protocol_runtime::udp::state::managed) struct ManagedDatagramState {
    handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>,
}

impl ManagedDatagramState {
    pub(in crate::protocol_runtime::udp::state::managed) fn new() -> Self {
        let handlers: Vec<Box<dyn ManagedDatagramFlowHandler>> = vec![
            #[cfg(feature = "shadowsocks")]
            (Box::new(SsChainManager::new()) as Box<dyn ManagedDatagramFlowHandler>),
            #[cfg(feature = "hysteria2")]
            (Box::new(H2ChainManager::new()) as Box<dyn ManagedDatagramFlowHandler>),
        ];
        Self { handlers }
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Option<Result<usize, FlowFailure>> {
        for handler in &mut self.handlers {
            if !handler.supports_managed_existing(&flow.resume) {
                continue;
            }
            return Some(
                handler
                    .send_managed_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                    .await,
            );
        }
        None
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
