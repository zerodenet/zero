use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use zero_core::Address;
use zero_engine::{EngineError, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;

use crate::logging::log_session_finished;
use crate::protocol_runtime::socks5_udp::{
    ClosedSocks5UdpAssociation, Socks5UdpAssociationView, Socks5UdpRuntime,
};
use crate::protocol_runtime::udp::{ChainTask, ProtocolUdpState};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;
use crate::runtime::udp_flow::sessions::UdpSessionFlows;
use crate::runtime::udp_helpers::send_direct_udp_packet;

impl UdpDispatch {
    /// Create a new dispatcher with an ephemeral direct socket.
    pub(crate) async fn new(inbound_tag: &str) -> Result<Self, EngineError> {
        let direct_socket = TokioDatagramSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5: Socks5UdpRuntime::default(),
            protocol_state: ProtocolUdpState::new(),
            managed_handles: HashMap::new(),
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
            socks5: Socks5UdpRuntime::default(),
            protocol_state: ProtocolUdpState::new(),
            managed_handles: HashMap::new(),
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

    pub(crate) fn protocol_parts(&mut self) -> (&mut ProtocolUdpState, &mut JoinSet<ChainTask>) {
        (&mut self.protocol_state, &mut self.chain_tasks)
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        &Socks5UdpRuntime,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            &self.direct_socket,
            &self.socks5,
            self.socks5.idle_deadline(),
            &mut self.chain_tasks,
        )
    }

    /// View of the SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn socks5_upstream_view(&self) -> Option<Socks5UdpAssociationView<'_>> {
        self.socks5.upstream_view()
    }

    /// The SOCKS5 idle deadline.
    #[allow(dead_code)]
    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.socks5.idle_deadline()
    }

    /// Update the SOCKS5 idle deadline (called after each send / recv).
    #[allow(dead_code)]
    pub(crate) fn touch_socks5_idle(&mut self, timeout: std::time::Duration) {
        self.socks5.touch_idle(timeout);
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
    pub(crate) fn drop_socks5_upstream(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.socks5.close_dropped()
    }

    /// Close the SOCKS5 upstream association on idle timeout.
    #[allow(dead_code)]
    pub(crate) fn close_socks5_idle(&mut self) {
        use crate::logging::log_udp_upstream_association_idle_timeout;
        if let Some(closed) = self.socks5.close_idle() {
            log_udp_upstream_association_idle_timeout(
                &self.inbound_tag,
                &closed.outbound_tag,
                &closed.server,
                closed.port,
                std::time::Duration::default(),
            );
        }
    }

    pub(crate) fn drop_socks5_idle(&mut self) -> Option<ClosedSocks5UdpAssociation> {
        self.socks5.close_idle()
    }

    /// Finish all tracked flows and close upstreams.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        self.socks5.close_all();

        for (_key, (session, mut handle)) in self.managed_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session;
            }
        }

        self.flows.finish_all()
    }
}
