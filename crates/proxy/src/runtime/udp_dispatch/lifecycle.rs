use std::net::SocketAddr;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;
use crate::runtime::udp_flow::sessions::UdpSessionFlows;
use crate::runtime::udp_flow::state::{UdpFlowState, UpstreamUdpPoll};
use crate::runtime::udp_helpers::send_direct_udp_packet;

pub(crate) struct UpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

impl UdpDispatch {
    /// Create a new dispatcher with an ephemeral direct socket.
    pub(crate) async fn new(inbound_tag: &str) -> Result<Self, EngineError> {
        let direct_socket = TokioDatagramSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            flow_state: UdpFlowState::default_registered(),
        })
    }

    /// Create a new dispatcher with a pre-bound direct socket.
    #[allow(dead_code)]
    pub(crate) fn with_socket(inbound_tag: &str, direct_socket: TokioDatagramSocket) -> Self {
        Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            flow_state: UdpFlowState::default_registered(),
        }
    }

    /// The direct outbound socket. Inbound handlers poll this for direct
    /// responses and use [`direct_response_session_id`] for metering.
    #[allow(dead_code)]
    pub(crate) fn direct_socket(&self) -> &TokioDatagramSocket {
        &self.direct_socket
    }

    /// Send a direct UDP packet through the dispatch-owned socket.
    pub(crate) async fn send_direct_packet(
        &self,
        target_addr: SocketAddr,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        send_direct_udp_packet(&self.direct_socket, target_addr, payload).await
    }

    /// Borrow direct socket and chain_tasks for `select!` polling.
    pub(crate) fn poll_sockets(&mut self) -> (&TokioDatagramSocket, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, self.flow_state.chain_tasks())
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        UpstreamUdpPoll<'_>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        let (upstream_udp, socks5_idle, chain_tasks) = self.flow_state.poll_refs();
        (&self.direct_socket, upstream_udp, socks5_idle, chain_tasks)
    }

    /// View of the SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn upstream_association_view(&self) -> Option<UpstreamAssociationView<'_>> {
        self.flow_state
            .upstream_association_view()
            .map(|association| UpstreamAssociationView {
                outbound_tag: association.outbound_tag,
            })
    }

    pub(crate) fn touch_upstream_idle(&mut self, timeout: std::time::Duration) {
        self.flow_state.touch_upstream_idle(timeout);
    }

    /// Look up the session ID for a direct response sender.
    pub(crate) fn direct_response_session_id(&self, sender: SocketAddr) -> Option<u64> {
        self.flows.direct_response_session_id(sender)
    }

    /// Look up a session ID by target+port only, regardless of outbound type.
    pub(crate) fn session_id_by_target(
        &self,
        target: &Address,
        port: u16,
        client_session_id: Option<u64>,
    ) -> Option<u64> {
        self.flows
            .session_id_by_target(target, port, client_session_id)
    }

    /// Look up the session ID for an upstream response (requires outbound tag).
    pub(crate) fn upstream_response_session_id(
        &self,
        outbound_tag: &str,
        target: &Address,
        port: u16,
    ) -> Option<u64> {
        self.flows
            .upstream_response_session_id(outbound_tag, target, port)
    }

    /// Drop the SOCKS5 upstream association after a receive error.
    pub(crate) fn drop_upstream_association(&mut self) -> Option<ClosedUpstreamAssociation> {
        self.flow_state
            .drop_upstream_association()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    pub(crate) fn drop_idle_upstream_association(&mut self) -> Option<ClosedUpstreamAssociation> {
        self.flow_state
            .close_idle_upstream()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    /// Finish all tracked flows and close upstreams.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        self.flow_state.close_all_upstreams();

        self.flows.finish_all()
    }
}
