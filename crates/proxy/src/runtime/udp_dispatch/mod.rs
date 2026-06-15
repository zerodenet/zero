//! Generic UDP dispatch: protocol-agnostic routing and outbound dispatch.
//!
//! [`UdpDispatch`] is the UDP pipe state machine.
//! Inbound protocols create one dispatcher per UDP association/session, submit
//! packets through [`crate::runtime::pipe::UdpPipe`], and poll this dispatcher
//! for responses to deliver to the client.
//!
//! # Module layout
//!
//! - [`forward`]: re-dispatch packets on existing outbound flows
//! - [`start`]: establish new outbound flows (single-hop and relay chains)
//! - [`ss_manager`]: Shadowsocks direct datagram manager
//! - [`h2_manager`]: Hysteria2 QUIC datagram manager
//! - [`trojan_manager`]: Trojan stream-packet manager
//! - [`mieru_manager`]: Mieru stream-packet manager
//! - [`packet_path_chain`]: generic datagram-over-packet-path manager for
//!   relay chains (Shadowsocks -> Shadowsocks, SOCKS5 -> Shadowsocks, etc.)
//!
//! # Supported outbounds
//!
//! All outbound types: direct, block, socks5, vless, shadowsocks, hysteria2,
//! trojan, mieru.
//!
//! # UDP relay chain model
//!
//! The relay chain model is:
//!
//! ```text
//! previous hop provides a packet path (send/recv raw payloads)
//! next hop encodes its protocol datagram through that path
//! ```
//!
//! Adding new datagram-over-packet-path combinations requires implementing
//! [`UdpPacketPath`] and [`DatagramCodec`], not creating protocol-pair modules.
//!
//! # Usage
//!
//! ```ignore
//! let mut dispatch = UdpDispatch::new("inbound-tag").await?;
//!
//! // For each incoming packet:
//! UdpPipe::new(proxy, &mut dispatch).dispatch(input).await?;
//!
//! // Poll for responses in a select loop:
//! select! {
//!     recv = dispatch.direct_socket().recv_from_addr(&mut buf) => { /* direct response */ }
//!     resp = dispatch.vless_manager().next_response() => { /* VLESS chain */ }
//!     // ...
//! }
//!
//! // Cleanup:
//! for completed in dispatch.finish_all() {
//!     log_completed_udp_flow(completed);
//! }
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;

use zero_core::{Address, Network, ProtocolType, Session, SessionAuth};
use zero_engine::{EngineError, ResolvedLeafOutbound, SessionHandle, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::UdpPacketFraming;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::udp_associate::sessions::{
    CompletedUdpFlow, UdpFlowOutbound, UdpFlowSnapshot, UdpSessionFlows,
};
use crate::runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::runtime::vmess_udp::VmessUdpOutboundManager;
use crate::runtime::Proxy;

// Sub-module declarations.

mod forward;
mod start;

mod packet_path_traits;

mod h2_manager;
mod mieru_manager;
#[cfg(feature = "shadowsocks")]
mod packet_path_chain;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod trojan_manager;

// Re-exports.

use h2_manager::H2ChainManager;
use mieru_manager::MieruChainManager;
#[cfg(feature = "shadowsocks")]
use packet_path_chain::PacketPathManager;
pub(crate) use packet_path_traits::ChainTask;
pub(super) use packet_path_traits::{DatagramCodec, UdpPacketPath};
use packet_path_traits::{
    H2UdpPeer, MieruUdpPeer, SsUdpPeer, TrojanUdpPeer, UdpFlowContext, UdpPacketRef,
    UdpPeerEndpoint,
};
#[cfg(feature = "shadowsocks")]
use ss_manager::SsChainManager;
use trojan_manager::TrojanChainManager;

// Types.

/// Result of starting a new UDP flow.
enum FlowStartResult {
    /// A new flow was established and tracked in `UdpSessionFlows`.
    Flow {
        outbound: UdpFlowOutbound,
        tx_bytes: u64,
    },
    /// A VLESS chain flow was established (tracked by the manager, not `UdpSessionFlows`).
    VlessFlow { session_id: u64, tag: String },
    /// A VMess UDP flow was established (tracked by the manager, not `UdpSessionFlows`).
    #[cfg(feature = "vmess")]
    VmessFlow { session_id: u64, tag: String },
    /// The target was blocked.
    Blocked { tag: String },
}

enum UdpCandidate<'a> {
    Leaf(ResolvedLeafOutbound<'a>),
    Relay(Vec<ResolvedLeafOutbound<'a>>),
}

/// Failure details for a flow start attempt.
struct FlowFailure {
    stage: &'static str,
    error: EngineError,
    upstream: Option<(String, u16)>,
}

// UdpDispatch.

/// Protocol-agnostic UDP dispatch state.
///
/// Owns all outbound-specific state (direct socket, upstream associations,
/// VLESS manager) and session flow tracking.  Created per inbound UDP
/// session/association.
pub(crate) struct UdpDispatch {
    inbound_tag: String,
    flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    direct_socket: TokioDatagramSocket,
    /// SOCKS5 upstream association (shared across all flows in this session).
    socks5_upstream: Option<crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation>,
    socks5_idle_deadline: Option<TokioInstant>,
    /// VLESS upstream manager (per-target connections).
    vless_manager: VlessUdpOutboundManager,
    /// VMess upstream manager (per-target connections).
    #[cfg(feature = "vmess")]
    vmess_manager: VmessUdpOutboundManager,
    /// Session handles for VLESS chain flows. These are not tracked by
    /// [`UdpSessionFlows`] because the VLESS manager owns the per-target
    /// upstream connections. We store handles here so `finish_all()` can
    /// properly complete them.
    vless_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Session handles for VMess UDP flows owned by the VMess manager.
    #[cfg(feature = "vmess")]
    vmess_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Unified JoinSet for chain-outbound (SS/H2/Trojan/Mieru/VLESS)
    /// response bridge tasks. Polled by [`poll_chain_response`].
    chain_tasks: JoinSet<ChainTask>,
    /// Per-dispatcher SS chain manager. Caches upstream sockets.
    #[cfg(feature = "shadowsocks")]
    ss_manager: SsChainManager,
    /// Per-dispatcher datagram-over-packet-path manager for UDP relay chains.
    /// Caches packet path carrier connections.
    #[cfg(feature = "shadowsocks")]
    packet_path_manager: PacketPathManager,
    /// Per-dispatcher Trojan chain manager. Caches TLS upstream streams.
    trojan_manager: TrojanChainManager,
    /// Per-dispatcher Mieru chain manager. Caches encrypted upstream streams.
    mieru_manager: MieruChainManager,
    /// Per-dispatcher H2 chain manager. Caches QUIC upstream connections.
    h2_manager: H2ChainManager,
}

impl UdpDispatch {
    /// Create a new dispatcher with an ephemeral direct socket.
    pub(crate) async fn new(inbound_tag: &str) -> Result<Self, EngineError> {
        let direct_socket = TokioDatagramSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess_manager: VmessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            #[cfg(feature = "vmess")]
            vmess_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(feature = "shadowsocks")]
            packet_path_manager: PacketPathManager::new(),
            trojan_manager: TrojanChainManager::new(),
            mieru_manager: MieruChainManager::new(),
            h2_manager: H2ChainManager::new(),
        })
    }

    /// Create a new dispatcher with a pre-bound direct socket.
    #[allow(dead_code)]
    pub(crate) fn with_socket(inbound_tag: &str, direct_socket: TokioDatagramSocket) -> Self {
        Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess_manager: VmessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            #[cfg(feature = "vmess")]
            vmess_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(feature = "shadowsocks")]
            packet_path_manager: PacketPathManager::new(),
            trojan_manager: TrojanChainManager::new(),
            mieru_manager: MieruChainManager::new(),
            h2_manager: H2ChainManager::new(),
        }
    }

    // Accessors.

    /// The direct outbound socket. Inbound handlers poll this for direct
    /// responses and use [`direct_response_session_id`] for metering.
    #[allow(dead_code)]
    pub(crate) fn direct_socket(&self) -> &TokioDatagramSocket {
        &self.direct_socket
    }

    /// Borrow direct socket and chain_tasks for `select!` polling.
    pub(crate) fn poll_sockets(&mut self) -> (&TokioDatagramSocket, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, &mut self.chain_tasks)
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    ///
    /// Returns:
    /// - `&TokioDatagramSocket`: direct outbound response socket
    /// - `Option<&ActiveUpstreamSocks5UdpAssociation>`: SOCKS5 chain upstream
    /// - `Option<TokioInstant>`: SOCKS5 idle deadline
    /// - `&mut JoinSet<ChainTask>`: unified chain response tasks
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        Option<&crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            &self.direct_socket,
            self.socks5_upstream.as_ref(),
            self.socks5_idle_deadline,
            &mut self.chain_tasks,
        )
    }

    /// The SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn socks5_upstream(
        &self,
    ) -> Option<&crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_upstream.as_ref()
    }

    /// The SOCKS5 idle deadline.
    #[allow(dead_code)]
    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.socks5_idle_deadline
    }

    /// Update the SOCKS5 idle deadline (called after each send / recv).
    #[allow(dead_code)]
    pub(crate) fn touch_socks5_idle(&mut self, timeout: std::time::Duration) {
        self.socks5_idle_deadline = Some(TokioInstant::now() + timeout);
    }

    /// Look up the session ID for a direct response sender.
    pub(crate) fn direct_response_session_id(&self, sender: SocketAddr) -> Option<u64> {
        self.flows.direct_response_session_id(sender)
    }

    /// Look up a session ID by target+port only, regardless of outbound type.
    ///
    /// Used for chain-outbound response metering where the outbound tag
    /// may not be known at the call site.
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

    /// Take and close the SOCKS5 upstream association (for idle timeout / error).
    #[allow(dead_code)]
    pub(crate) fn take_socks5_upstream(
        &mut self,
    ) -> Option<crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_idle_deadline = None;
        self.socks5_upstream.take()
    }

    /// Close the SOCKS5 upstream association on idle timeout.
    #[allow(dead_code)]
    pub(crate) fn close_socks5_idle(&mut self) {
        use crate::logging::log_udp_upstream_association_idle_timeout;
        use crate::outbound::socks5::UpstreamAssociationCloseReason;

        if let Some(assoc) = self.socks5_upstream.take() {
            let outbound_tag = assoc.outbound_tag().to_owned();
            let (server, port) = assoc.upstream_endpoint();
            let server = server.to_owned();
            assoc.close(UpstreamAssociationCloseReason::IdleTimeout);
            log_udp_upstream_association_idle_timeout(
                &self.inbound_tag,
                &outbound_tag,
                &server,
                port,
                std::time::Duration::default(), // caller should log with actual timeout
            );
            self.socks5_idle_deadline = None;
        }
    }

    /// Finish all tracked flows and close upstreams.
    ///
    /// Closes the SOCKS5 upstream association, finishes VLESS chain flow
    /// session handles, and drains all regular flows from `UdpSessionFlows`.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        if let Some(assoc) = self.socks5_upstream {
            use crate::outbound::socks5::UpstreamAssociationCloseReason;
            assoc.close(UpstreamAssociationCloseReason::Closed);
        }

        // Finish VLESS chain flow session handles.
        for (_key, (session, mut handle)) in self.vless_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session; // session was moved into the record
            }
        }

        #[cfg(feature = "vmess")]
        for (_key, (session, mut handle)) in self.vmess_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session;
            }
        }

        self.flows.finish_all()
    }

    // Dispatch.

    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port, client_session_id)`
    /// (including VLESS chain connections cached in the manager), forwards the
    /// payload.  Otherwise creates a new session, routes through the engine,
    /// and dispatches to the resolved outbound.
    ///
    /// `client_session_id`: when `Some`, isolates flows that would otherwise
    /// collide on `(target, port)` alone (SIP022 3.2.4 per-client-session
    /// routing).  All non-SS inbound callers pass `None`.
    ///
    /// Supports all outbound types: direct, block, socks5, vless,
    /// shadowsocks, hysteria2, trojan, mieru.
    /// Returns the session_id of the (new or cached) flow for metering.
    pub(crate) async fn dispatch(
        &mut self,
        proxy: &Proxy,
        target: Address,
        port: u16,
        payload: &[u8],
        protocol: ProtocolType,
        auth: Option<&SessionAuth>,
        client_session_id: Option<u64>,
    ) -> Result<u64, EngineError> {
        // VLESS manager shortcut (cached upstream).
        if let Some(handle) = self.vless_manager.get(&target, port) {
            proxy.record_session_inbound_rx(handle.session_id, payload.len() as u64);
            let packet = <vless::VlessOutbound as UdpPacketFraming<
                vless::VlessUdpPacketTarget,
            >>::encode_udp_packet(
                &proxy.protocols.vless_outbound,
                &vless::VlessUdpPacketTarget {
                    address: &target,
                    port,
                    payload,
                },
            )?;
            let packet_len = packet.len() as u64;
            let _ = handle.send_tx.send(packet).await;
            proxy.record_session_outbound_tx(handle.session_id, packet_len);
            // Spawn bridge task for the expected response.
            self.vless_manager
                .spawn_bridge(&mut self.chain_tasks, target, port, handle.session_id);
            return Ok(handle.session_id);
        }

        #[cfg(feature = "vmess")]
        if let Some(handle) = self.vmess_manager.get(&target, port) {
            proxy.record_session_inbound_rx(handle.session_id, payload.len() as u64);
            let packet = <vmess::VmessOutbound as UdpPacketFraming<
                vmess::VmessUdpPacketTarget,
            >>::encode_udp_packet(
                &proxy.protocols.vmess_outbound,
                &vmess::VmessUdpPacketTarget {
                    address: &target,
                    port,
                    payload,
                },
            )?;
            let packet_len = packet.len() as u64;
            let _ = handle.send_tx.send(packet).await;
            proxy.record_session_outbound_tx(handle.session_id, packet_len);
            self.vmess_manager
                .spawn_bridge(&mut self.chain_tasks, target, port, handle.session_id);
            return Ok(handle.session_id);
        }

        // Existing flow (direct / socks5 / ss / h2 / trojan / mieru).
        if let Some(flow) = self.flows.snapshot(&target, port, client_session_id) {
            self.forward_existing(proxy, &flow, payload).await?;
            return Ok(flow.session.id);
        }

        // New flow.
        let mut session = Session::new(0, target, port, Network::Udp, protocol);
        if let Some(a) = auth {
            session.auth = Some(a.clone());
        }
        proxy.prepare_session(&mut session, &self.inbound_tag, None);
        let mut session_handle = proxy.track_session(session.id);
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(session.id, payload.len() as u64);

        proxy.resolve_fake_ip_target(&mut session).await;
        let action = proxy.route_decision(&session);
        let resolved = match proxy.resolve_outbound(&action) {
            Ok((resolved, _plan)) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, proxy.config.mode.kind());

        let candidates = match resolved {
            ResolvedOutbound::Single(c) => vec![UdpCandidate::Leaf(c)],
            ResolvedOutbound::Fallback { candidates } => {
                candidates.into_iter().map(UdpCandidate::Leaf).collect()
            }
            ResolvedOutbound::Relay { chain } => vec![UdpCandidate::Relay(chain)],
        };
        let is_fallback = candidates.len() > 1;
        let mut last_failure = None;

        for candidate in candidates {
            match self.start_flow(proxy, candidate, &session, payload).await {
                Ok(FlowStartResult::Flow { outbound, tx_bytes }) => {
                    let session_id = session.id;
                    session.outbound_tag = Some(outbound.tag().to_owned());
                    proxy.set_session_outbound(&session);
                    self.flows
                        .insert(session, session_handle, outbound, client_session_id);
                    proxy.record_session_outbound_tx(session_id, tx_bytes);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::VlessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vless_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                    return Ok(session_id);
                }
                #[cfg(feature = "vmess")]
                Ok(FlowStartResult::VmessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vmess_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::Blocked { tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                        log_session_finished(&record, None);
                    }
                    return Ok(session.id);
                }
                Err(failure) => {
                    last_failure = Some(failure);
                }
            }
        }

        let record = session_handle.finish(SessionOutcome::Failed);
        if let Some(failure) = last_failure {
            let stage = if is_fallback {
                "fallback_exhausted"
            } else {
                failure.stage
            };
            log_session_failed(
                &session,
                record.as_ref(),
                stage,
                started_at.elapsed(),
                &failure.error,
                failure
                    .upstream
                    .as_ref()
                    .map(|(server, port)| (server.as_str(), *port)),
            );
            return Err(failure.error);
        }

        let error = EngineError::Io(std::io::Error::other("all fallback outbounds failed"));
        log_session_failed(
            &session,
            record.as_ref(),
            "fallback_exhausted",
            started_at.elapsed(),
            &error,
            None,
        );
        Err(error)
    }

    // SOCKS5 helper.

    /// Send via SOCKS5 upstream association, establishing one if needed.
    async fn send_socks5(
        &mut self,
        proxy: &Proxy,
        tag: &str,
        server: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        use crate::logging::log_udp_upstream_association_dropped;
        use crate::outbound::socks5::{
            send_socks5_udp_packet, Socks5UdpAssociation, UpstreamAssociationCloseReason,
        };

        let association = Socks5UdpAssociation {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            auth: username
                .zip(password)
                .map(|(u, p)| (u.to_owned(), p.to_owned())),
        };

        match send_socks5_udp_packet(
            proxy,
            &self.inbound_tag,
            &association,
            session,
            payload,
            &mut self.socks5_upstream,
            &mut self.socks5_idle_deadline,
        )
        .await
        {
            Ok(sent) => {
                // packet_sent already recorded in send_socks5_udp_packet
                Ok(sent)
            }
            Err(error) => {
                if let Some(assoc) = self.socks5_upstream.take() {
                    let outbound_tag = assoc.outbound_tag().to_owned();
                    let (svr, p) = assoc.upstream_endpoint();
                    let svr = svr.to_owned();
                    assoc.close(UpstreamAssociationCloseReason::Dropped);
                    log_udp_upstream_association_dropped(
                        &self.inbound_tag,
                        &outbound_tag,
                        &svr,
                        p,
                        &error,
                    );
                }
                self.socks5_idle_deadline = None;
                proxy.record_udp_upstream_send_failure();
                Err(error)
            }
        }
    }

    // Failure helpers.

    fn fail_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        error: &EngineError,
    ) {
        if let Some(completed) = self.flows.finish(
            &flow.session.target,
            flow.session.port,
            flow.client_session_id,
            SessionOutcome::Failed,
        ) {
            log_session_failed(
                &flow.session,
                Some(&completed.record),
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        } else {
            log_session_failed(
                &flow.session,
                None,
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        }
    }

    fn fail_flow_with_msg(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        msg: &str,
    ) {
        let error = EngineError::Io(std::io::Error::other(msg.to_string()));
        self.fail_flow(flow, started_at, stage, &error);
    }

    /// Record outbound bytes or fail the flow, for the common
    /// manager-based dispatch pattern in [`forward_existing()`].
    fn record_or_fail(
        &mut self,
        flow: &UdpFlowSnapshot,
        proxy: &Proxy,
        started_at: Instant,
        result: Result<usize, FlowFailure>,
    ) -> Result<(), EngineError> {
        match result {
            Ok(sent) => {
                proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                Ok(())
            }
            Err(failure) => {
                self.fail_flow_with_msg(
                    flow,
                    started_at,
                    failure.stage,
                    &failure.error.to_string(),
                );
                Err(failure.error)
            }
        }
    }
}

use zero_engine::ResolvedOutbound;
