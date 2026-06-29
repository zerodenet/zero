use super::model::{ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler};
use super::state::flow_mismatch;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::{
    ManagedRelayStreamFlow, ManagedStreamPacketFlow, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

use super::stream_sender::{ManagedStreamFlowSender, ManagedStreamSenderState};

fn stream_sender_unavailable(stage: &'static str, message: &'static str) -> FlowFailure {
    FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::other(message)),
        upstream: None,
    }
}

pub(super) struct ManagedStreamState {
    handlers: Vec<Box<dyn ManagedStreamFlowHandler>>,
    senders: ManagedStreamSenderState,
}

impl ManagedStreamState {
    pub(super) fn new(handlers: Vec<Box<dyn ManagedStreamFlowHandler>>) -> Self {
        Self {
            handlers,
            senders: ManagedStreamSenderState::new(),
        }
    }

    pub(super) fn register_sender(
        &mut self,
        flow_ref: ManagedUdpFlowRef,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) {
        self.senders.push_sender(flow_ref, sender);
    }

    pub(super) async fn forward_registered_sender(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow_ref: ManagedUdpFlowRef,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let sender = self.senders.sender(flow_ref)?;
        Some(
            sender
                .send_existing(
                    chain_tasks,
                    proxy,
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_stream_send",
                    error,
                    upstream: None,
                })
                .and_then(|session_id| {
                    session_id.map(|_| payload.len()).ok_or_else(|| {
                        stream_sender_unavailable(
                            "udp_stream_send",
                            "managed stream UDP flow was dropped",
                        )
                    })
                }),
        )
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
