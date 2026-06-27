use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    ManagedStreamFlowSender, ManagedUdpFlowRequest, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::{PacketPathManager, SendWithSnapshotRequest};
use crate::runtime::udp_flow::registered::{
    ClosedRegisteredUpstreamAssociation, RegisteredUdpHandlers, RegisteredUdpState,
    RegisteredUpstreamAssociationView,
};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(crate) struct UdpFlowState {
    registered: RegisteredUdpState,
    packet_path: PacketPathManager,
    chain_tasks: JoinSet<ChainTask>,
}

pub(crate) struct UpstreamUdpPoll<'a> {
    registered: &'a RegisteredUdpState,
}

impl UpstreamUdpPoll<'_> {
    pub(crate) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.registered.recv_upstream_packet(buf).await
    }
}

impl UdpFlowState {
    pub(crate) fn new(handlers: RegisteredUdpHandlers) -> Self {
        Self {
            registered: RegisteredUdpState::new(handlers),
            packet_path: PacketPathManager::new(),
            chain_tasks: JoinSet::new(),
        }
    }

    pub(crate) fn default_registered() -> Self {
        Self::new(crate::register::registered_udp_handlers())
    }

    pub(crate) fn chain_tasks(&mut self) -> &mut JoinSet<ChainTask> {
        &mut self.chain_tasks
    }

    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        UpstreamUdpPoll<'_>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            UpstreamUdpPoll {
                registered: &self.registered,
            },
            self.registered.upstream_idle_deadline(),
            &mut self.chain_tasks,
        )
    }

    pub(crate) fn upstream_association_view(
        &self,
    ) -> Option<RegisteredUpstreamAssociationView<'_>> {
        self.registered.upstream_association_view()
    }

    pub(crate) fn touch_upstream_idle(&mut self, timeout: std::time::Duration) {
        self.registered.touch_upstream_idle(timeout);
    }

    pub(crate) fn drop_upstream_association(
        &mut self,
    ) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.registered.drop_upstream_association()
    }

    pub(crate) fn close_idle_upstream(&mut self) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.registered.close_idle_upstream()
    }

    pub(crate) fn close_all_upstreams(self) {
        self.registered.close_all_upstreams();
    }

    pub(crate) fn register_managed_stream_flow_sender(
        &mut self,
        sender: Box<dyn ManagedStreamFlowSender>,
    ) -> ManagedUdpFlowRef {
        self.registered.register_managed_stream_flow_sender(sender)
    }

    pub(crate) async fn start_managed_flow(
        &mut self,
        inbound_tag: &str,
        mut request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        request.chain_tasks = Some(&mut self.chain_tasks);
        self.registered
            .start_managed_udp_flow(inbound_tag, request)
            .await
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.registered.register_managed_flow(resume)
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.registered.managed_flow_resume(flow_ref)
    }

    pub(crate) async fn forward_existing_managed_flow(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.registered
            .forward_existing_managed_flow(&mut self.chain_tasks, proxy, flow, payload)
            .await
    }

    pub(crate) async fn send_packet_path_chain(
        &mut self,
        session_id: u64,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(
                UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id,
                },
                proxy,
                carrier_leaf,
                datagram_leaf,
                packet,
            )
            .await
    }

    pub(crate) async fn forward_existing_packet_path_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        let snapshot = flow
            .outbound
            .packet_path_snapshot()
            .expect("packet-path flow should expose packet-path snapshot");
        self.packet_path
            .send_with_snapshot(SendWithSnapshotRequest {
                ctx: UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id: flow.session.id,
                },
                lookup_key: snapshot.lookup_key(),
                packet_ref: UdpPacketRef {
                    target: &flow.session.target,
                    port: flow.session.port,
                    payload,
                },
            })
            .await
    }
}
