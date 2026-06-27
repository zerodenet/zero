use super::model::{ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler};
use super::state::flow_mismatch;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    ManagedRelayStreamFlow, ManagedStreamPacketFlow, ManagedUdpFlowSnapshot,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

pub(super) struct ManagedStreamState {
    handlers: Vec<Box<dyn ManagedStreamFlowHandler>>,
}

impl ManagedStreamState {
    pub(super) fn new(handlers: Vec<Box<dyn ManagedStreamFlowHandler>>) -> Self {
        Self { handlers }
    }

    pub(super) async fn start_stream_packet_flow(
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

    pub(super) async fn start_relay_stream_flow(
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
