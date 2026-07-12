use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::{ManagedDatagramFlow, ManagedUdpFlowResume};
use crate::runtime::udp_flow::managed::model::{ManagedDatagramFlowHandler, ManagedExistingSend};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

pub(in crate::runtime::udp_flow::managed) struct ManagedDatagramState {
    handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>,
}

impl ManagedDatagramState {
    pub(in crate::runtime::udp_flow::managed) fn new(
        handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    ) -> Self {
        Self { handlers }
    }

    pub(in crate::runtime::udp_flow::managed) async fn start_datagram_flow(
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

    pub(in crate::runtime::udp_flow::managed) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        resume: &ManagedUdpFlowResume,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
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
