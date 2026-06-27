use super::model::{ManagedDatagramFlowHandler, ManagedExistingSend};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{ManagedDatagramFlow, ManagedUdpFlowSnapshot};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

pub(super) struct ManagedDatagramState {
    handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>,
}

impl ManagedDatagramState {
    pub(super) fn new(handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>) -> Self {
        Self { handlers }
    }

    pub(super) async fn start_datagram_flow(
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

    pub(super) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ManagedUdpFlowSnapshot,
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
