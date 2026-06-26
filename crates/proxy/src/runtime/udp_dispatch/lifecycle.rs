use std::net::SocketAddr;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::protocol_runtime::socks5_udp::Socks5UdpRuntime;
use crate::protocol_runtime::udp::ProtocolUdpState;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::managed::ManagedUdpFlows;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path_chain::PacketPathManager;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;
use crate::runtime::udp_flow::sessions::UdpSessionFlows;
use crate::runtime::udp_helpers::send_direct_udp_packet;

pub(crate) struct UpstreamUdpPoll<'a> {
    socks5: &'a Socks5UdpRuntime,
}

impl UpstreamUdpPoll<'_> {
    pub(crate) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.socks5.recv_upstream_packet(buf).await
    }
}

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
            protocol_state: ProtocolUdpState::new(),
            packet_path: PacketPathManager::new(),
            managed_flows: ManagedUdpFlows::default(),
            chain_tasks: JoinSet::new(),
        })
    }

    /// Create a new dispatcher with a pre-bound direct socket.
    #[allow(dead_code)]
    pub(crate) fn with_socket(inbound_tag: &str, direct_socket: TokioDatagramSocket) -> Self {
        Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            protocol_state: ProtocolUdpState::new(),
            packet_path: PacketPathManager::new(),
            managed_flows: ManagedUdpFlows::default(),
            chain_tasks: JoinSet::new(),
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
        (&self.direct_socket, &mut self.chain_tasks)
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
        (
            &self.direct_socket,
            UpstreamUdpPoll {
                socks5: self.protocol_state.socks5_runtime(),
            },
            self.protocol_state.socks5_idle_deadline(),
            &mut self.chain_tasks,
        )
    }

    /// View of the SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn upstream_association_view(&self) -> Option<UpstreamAssociationView<'_>> {
        self.protocol_state
            .socks5_upstream_view()
            .map(|association| UpstreamAssociationView {
                outbound_tag: association.outbound_tag,
            })
    }

    /// The SOCKS5 idle deadline.
    #[allow(dead_code)]
    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.protocol_state.socks5_idle_deadline()
    }

    /// Update the SOCKS5 idle deadline (called after each send / recv).
    #[allow(dead_code)]
    pub(crate) fn touch_socks5_idle(&mut self, timeout: std::time::Duration) {
        self.protocol_state.touch_socks5_idle(timeout);
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
        self.protocol_state
            .drop_socks5_upstream()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    pub(crate) fn drop_idle_upstream_association(&mut self) -> Option<ClosedUpstreamAssociation> {
        self.protocol_state
            .close_socks5_idle()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    /// Finish all tracked flows and close upstreams.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        self.protocol_state.close_socks5_all();

        self.managed_flows.finish_all();

        self.flows.finish_all()
    }
}
