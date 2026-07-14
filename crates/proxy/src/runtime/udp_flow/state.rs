use tokio::task::JoinSet;
#[cfg(feature = "socks5")]
use tokio::time::Instant as TokioInstant;
#[cfg(feature = "socks5")]
use zero_engine::EngineError;

use crate::protocol_registry::UdpAdapterContext;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedExistingFlowForward;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::packet_path::{ChainTask, UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::{
    PacketPathManager, PacketPathStartRequest, SendWithSnapshotRequest,
};
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::ClosedRegisteredUpstreamAssociation;
use crate::runtime::udp_flow::registered::{RegisteredUdpHandlers, RegisteredUdpState};
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::{
    RegisteredUpstreamAssociationView, UpstreamAssociationSend,
};
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;
pub(crate) struct UdpFlowState {
    registered: RegisteredUdpState,
    packet_path: PacketPathManager,
    chain_tasks: JoinSet<ChainTask>,
}

pub(crate) struct UdpFlowStartContext<'a> {
    inbound_tag: &'a str,
    state: &'a mut UdpFlowState,
}

impl<'a> UdpFlowStartContext<'a> {
    pub(crate) fn new(inbound_tag: &'a str, state: &'a mut UdpFlowState) -> Self {
        Self { inbound_tag, state }
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn start_managed_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.state
            .start_managed_flow(self.inbound_tag, request)
            .await
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.state.register_managed_flow(resume)
    }
}

#[cfg(feature = "socks5")]
pub(crate) struct UpstreamUdpPoll<'a> {
    registered: &'a RegisteredUdpState,
}

#[cfg(feature = "socks5")]
impl UpstreamUdpPoll<'_> {
    pub(crate) async fn recv_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        self.registered.recv_upstream_response(buf).await
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

    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        all(
            not(feature = "socks5"),
            any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            )
        )
    ))]
    pub(crate) fn chain_tasks(&mut self) -> &mut JoinSet<ChainTask> {
        &mut self.chain_tasks
    }

    #[cfg(feature = "socks5")]
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

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_association_view(
        &self,
    ) -> Option<RegisteredUpstreamAssociationView<'_>> {
        self.registered.upstream_association_view()
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn touch_upstream_idle(&mut self, timeout: std::time::Duration) {
        self.registered.touch_upstream_idle(timeout);
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn drop_upstream_association(
        &mut self,
    ) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.registered.drop_upstream_association()
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn close_idle_upstream(&mut self) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.registered.close_idle_upstream()
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn close_all_upstreams(self) {
        self.registered.close_all_upstreams();
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
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

    #[cfg(feature = "socks5")]
    pub(crate) async fn start_upstream_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .start_upstream_udp_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn handles_upstream_resume(&self, resume: &ManagedUdpFlowResume) -> bool {
        self.registered.handles_upstream_resume(resume)
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.registered.register_managed_flow(resume)
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.registered.managed_flow_resume(flow_ref)
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn forward_existing_managed_flow(
        &mut self,
        services: crate::protocol_registry::UdpRuntimeServices,
        request: ManagedExistingFlowForward<'_>,
    ) -> Result<usize, FlowFailure> {
        self.registered
            .forward_existing_managed_flow(&mut self.chain_tasks, services, request)
            .await
    }

    pub(crate) async fn send_packet_path_chain(
        &mut self,
        ctx: UdpAdapterContext<'_>,
        request: PacketPathStartRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(
                UdpFlowContext {
                    chain_tasks: &mut self.chain_tasks,
                    session_id: request.session_id,
                },
                ctx,
                request,
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
