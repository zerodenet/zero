//! Generic UDP dispatch: protocol-agnostic routing and outbound dispatch.
//!
//! [`UdpDispatch`] is the single entry point for all UDP packet routing.
//! Each inbound protocol creates a `UdpDispatch` instance, calls [`dispatch()`]
//! for each incoming packet, and polls for responses to deliver to its client.
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
//! dispatch.dispatch(proxy, target, port, payload, ProtocolType::Vless, auth.as_ref()).await?;
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
use zero_engine::{
    EngineError, ResolvedLeafOutbound, ResolvedOutbound, SessionHandle, SessionOutcome,
};
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::UdpPacketFraming;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
use crate::runtime::udp_associate::sessions::UdpPacketPathCarrier;
use crate::runtime::udp_associate::sessions::{
    CompletedUdpFlow, UdpFlowOutbound, UdpFlowSnapshot, UdpSessionFlows,
};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::vless_udp::{
    establish_vless_udp_upstream_over_stream, VlessUdpOutboundManager, VlessUdpTransport,
};
use crate::runtime::Proxy;

// ── Sub-module declarations ──────────────────────────────────────────

mod packet_path_traits;

mod h2_manager;
mod mieru_manager;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
mod packet_path_chain;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod trojan_manager;

// ── Re-exports ───────────────────────────────────────────────────────

use h2_manager::H2ChainManager;
use mieru_manager::MieruChainManager;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
use packet_path_chain::{PacketPathChainParams, PacketPathManager};
pub(crate) use packet_path_traits::ChainTask;
pub(super) use packet_path_traits::{DatagramCodec, UdpPacketPath};
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
    /// The target was blocked.
    Blocked { tag: String },
}

enum UdpCandidate<'a> {
    Leaf(ResolvedLeafOutbound<'a>),
    Relay(Vec<ResolvedLeafOutbound<'a>>),
}

/// Resolve a relay chain into packet-path + datagram parameters.
///
/// Returns `Some` when the chain matches the "packet path carrier → datagram
/// protocol" pattern. Currently recognises `[SOCKS5, Shadowsocks]`. Adding
/// new combinations only requires extending this function and implementing
/// [`UdpPacketPath`] + [`DatagramCodec`] — no new protocol-pair modules.
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
fn resolve_udp_packet_path_chain<'a>(
    chain: &[ResolvedLeafOutbound<'a>],
) -> Option<PacketPathChainParams<'a>> {
    match chain {
        [ResolvedLeafOutbound::Socks5 {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            username: carrier_username,
            password: carrier_password,
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier_tag,
            carrier_server,
            carrier_port: *carrier_port,
            carrier_username: *carrier_username,
            carrier_password: *carrier_password,
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        _ => None,
    }
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
    /// Session handles for VLESS chain flows. These are not tracked by
    /// [`UdpSessionFlows`] because the VLESS manager owns the per-target
    /// upstream connections. We store handles here so `finish_all()` can
    /// properly complete them.
    vless_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Unified JoinSet for chain-outbound (SS/H2/Trojan/Mieru/VLESS)
    /// response bridge tasks. Polled by [`poll_chain_response`].
    chain_tasks: JoinSet<ChainTask>,
    /// Per-dispatcher SS chain manager. Caches upstream sockets.
    #[cfg(feature = "shadowsocks")]
    ss_manager: SsChainManager,
    /// Per-dispatcher datagram-over-packet-path manager for UDP relay chains.
    /// Caches packet path carrier connections.
    #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
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
            vless_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
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
            vless_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
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
    pub(crate) fn session_id_by_target(&self, target: &Address, port: u16) -> Option<u64> {
        self.flows.session_id_by_target(target, port)
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

        self.flows.finish_all()
    }

    // Dispatch.

    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port)` (including VLESS chain
    /// connections cached in the manager), forwards the payload.  Otherwise
    /// creates a new session, routes through the engine, and dispatches to
    /// the resolved outbound.
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

        // Existing flow (direct / socks5 / ss / h2 / trojan / mieru).
        if let Some(flow) = self.flows.snapshot(&target, port) {
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
                    self.flows.insert(session, session_handle, outbound);
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

    /// Forward a packet to an existing flow.
    async fn forward_existing(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(flow.session.id, payload.len() as u64);

        match &flow.outbound {
            UdpFlowOutbound::Direct { target_addr, .. } => {
                match send_direct_udp_packet(&self.direct_socket, *target_addr, payload).await {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        self.fail_flow(&flow, started_at, "udp_direct_send", &error);
                        return Err(error);
                    }
                }
            }
            UdpFlowOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => match self
                .send_socks5(
                    proxy,
                    tag,
                    server,
                    *port,
                    username.as_deref(),
                    password.as_deref(),
                    &flow.session,
                    payload,
                )
                .await
            {
                Ok(sent) => {
                    proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                }
                Err(error) => {
                    self.fail_flow(&flow, started_at, "udp_upstream_send", &error);
                    return Err(error);
                }
            },
            #[cfg(feature = "shadowsocks")]
            UdpFlowOutbound::Shadowsocks {
                tag: _,
                server,
                port,
                password,
                cipher,
                packet_path_carrier,
            } => {
                #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
                let result = if let Some(carrier) = packet_path_carrier {
                    self.packet_path_manager
                        .send(
                            &mut self.chain_tasks,
                            flow.session.id,
                            proxy,
                            &PacketPathChainParams {
                                datagram_tag: "",
                                carrier_tag: carrier.tag.as_str(),
                                carrier_server: carrier.server.as_str(),
                                carrier_port: carrier.port,
                                carrier_username: carrier.username.as_deref(),
                                carrier_password: carrier.password.as_deref(),
                                datagram_server: server.as_str(),
                                datagram_port: *port,
                                datagram_password: password.as_str(),
                                datagram_cipher: cipher.as_str(),
                            },
                            &flow.session.target,
                            flow.session.port,
                            payload,
                        )
                        .await
                } else {
                    self.ss_manager
                        .send(
                            &mut self.chain_tasks,
                            flow.session.id,
                            server.as_str(),
                            *port,
                            password.as_str(),
                            cipher.as_str(),
                            &flow.session.target,
                            flow.session.port,
                            payload,
                        )
                        .await
                };

                #[cfg(all(not(feature = "socks5"), feature = "shadowsocks"))]
                let result = self
                    .ss_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        cipher.as_str(),
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await;

                match result {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "shadowsocks"))]
            UdpFlowOutbound::Shadowsocks { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Shadowsocks UDP outbound requires feature `shadowsocks`",
                )));
            }
            #[cfg(feature = "hysteria2")]
            UdpFlowOutbound::Hysteria2 {
                tag: _,
                server,
                port,
                password,
                client_fingerprint,
            } => {
                match self
                    .h2_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        client_fingerprint.as_deref(),
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "hysteria2"))]
            UdpFlowOutbound::Hysteria2 { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Hysteria2 UDP outbound requires feature `hysteria2`",
                )));
            }
            #[cfg(feature = "trojan")]
            UdpFlowOutbound::Trojan {
                tag: _,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
            } => {
                match self
                    .trojan_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        &flow.session,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        sni.as_deref(),
                        *insecure,
                        client_fingerprint.as_deref(),
                        *relay_chain,
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "trojan"))]
            UdpFlowOutbound::Trojan { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Trojan UDP outbound requires feature `trojan`",
                )));
            }
            #[cfg(feature = "mieru")]
            UdpFlowOutbound::Mieru {
                tag: _,
                server,
                port,
                username,
                password,
                relay_chain,
            } => {
                match self
                    .mieru_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        &flow.session,
                        server.as_str(),
                        *port,
                        username.as_str(),
                        password.as_str(),
                        *relay_chain,
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "mieru"))]
            UdpFlowOutbound::Mieru { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Mieru UDP outbound requires feature `mieru`",
                )));
            }
        }

        Ok(())
    }

    /// Start a new UDP flow by dispatching to the resolved outbound.
    async fn start_flow(
        &mut self,
        proxy: &Proxy,
        candidate: UdpCandidate<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidate = match candidate {
            UdpCandidate::Leaf(candidate) => candidate,
            UdpCandidate::Relay(chain) => {
                return self.start_relay_flow(proxy, chain, session, payload).await;
            }
        };

        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = proxy
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(session, proxy.resolver.as_ref())
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = self
                    .direct_socket
                    .send_to_addr(payload, target_addr)
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(FlowStartResult::Blocked {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .send_socks5(
                        proxy, tag, server, port, username, password, session, payload,
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_upstream_send",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                let transport = VlessUdpTransport {
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    quic,
                };
                let session_id = session.id;
                let tag_owned = tag.to_owned();
                self.vless_manager
                    .get_or_create_upstream(
                        &mut self.chain_tasks,
                        proxy,
                        session,
                        session.target.clone(),
                        session.port,
                        server.to_owned(),
                        port,
                        id.to_owned(),
                        payload.to_vec(),
                        Some(&transport),
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_vless_upstream",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "hysteria2")]
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                client_fingerprint,
                ..
            } => {
                let sent = self
                    .h2_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        server,
                        port,
                        password,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Hysteria2 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "hysteria2"))]
            ResolvedLeafOutbound::Hysteria2 { .. } => Err(FlowFailure {
                stage: "udp_hysteria2_outbound",
                error: zero_core::Error::Unsupported(
                    "Hysteria2 UDP outbound requires Cargo feature `hysteria2`",
                )
                .into(),
                upstream: None,
            }),
            #[allow(unused_variables)]
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
                ..
            } => {
                #[cfg(feature = "shadowsocks")]
                {
                    let sent = self
                        .ss_manager
                        .send(
                            &mut self.chain_tasks,
                            session.id,
                            server,
                            port,
                            password,
                            cipher,
                            &session.target,
                            session.port,
                            payload,
                        )
                        .await
                        .map_err(|f: FlowFailure| FlowFailure {
                            stage: f.stage,
                            error: f.error,
                            upstream: f.upstream,
                        })?;

                    Ok(FlowStartResult::Flow {
                        outbound: UdpFlowOutbound::Shadowsocks {
                            tag: tag.to_owned(),
                            server: server.to_owned(),
                            port,
                            password: password.to_owned(),
                            cipher: cipher.to_owned(),
                            packet_path_carrier: None,
                        },
                        tx_bytes: sent as u64,
                    })
                }
                #[cfg(not(feature = "shadowsocks"))]
                {
                    Err(FlowFailure {
                        stage: "udp_shadowsocks_outbound",
                        error: zero_core::Error::Unsupported(
                            "Shadowsocks UDP outbound requires Cargo feature `shadowsocks`",
                        )
                        .into(),
                        upstream: None,
                    })
                }
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        false,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: false,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Trojan { .. } => Err(FlowFailure {
                stage: "udp_trojan_outbound",
                error: zero_core::Error::Unsupported(
                    "Trojan UDP outbound requires Cargo feature `trojan`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .mieru_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        username,
                        password,
                        false,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
                        relay_chain: false,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "mieru"))]
            ResolvedLeafOutbound::Mieru { .. } => Err(FlowFailure {
                stage: "udp_mieru_outbound",
                error: zero_core::Error::Unsupported(
                    "Mieru UDP outbound requires Cargo feature `mieru`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
        }
    }

    async fn start_relay_flow(
        &mut self,
        proxy: &Proxy,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        // Datagram-over-packet-path: previous hop provides a packet path,
        // next hop encodes its datagram through it.
        #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
        if let Some(params) = resolve_udp_packet_path_chain(&chain) {
            let sent = self
                .packet_path_manager
                .send(
                    &mut self.chain_tasks,
                    session.id,
                    proxy,
                    &params,
                    &session.target,
                    session.port,
                    payload,
                )
                .await?;

            return Ok(FlowStartResult::Flow {
                outbound: UdpFlowOutbound::Shadowsocks {
                    tag: params.datagram_tag.to_owned(),
                    server: params.datagram_server.to_owned(),
                    port: params.datagram_port,
                    password: params.datagram_password.to_owned(),
                    cipher: params.datagram_cipher.to_owned(),
                    packet_path_carrier: Some(UdpPacketPathCarrier {
                        tag: params.carrier_tag.to_owned(),
                        server: params.carrier_server.to_owned(),
                        port: params.carrier_port,
                        username: params.carrier_username.map(ToOwned::to_owned),
                        password: params.carrier_password.map(ToOwned::to_owned),
                    }),
                },
                tx_bytes: sent as u64,
            });
        }

        let (stream, final_hop) = proxy
            .establish_relay_prefix(chain)
            .await
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream_endpoint,
            })?;

        match final_hop {
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                if quic.is_some() {
                    return Err(FlowFailure {
                        stage: "udp_relay_final_transport",
                        error: zero_core::Error::Unsupported(
                            "VLESS QUIC final hop over TCP relay chain is not supported",
                        )
                        .into(),
                        upstream: None,
                    });
                }

                let session_id = session.id;
                let tag_owned = tag.to_owned();
                let key = (session.target.clone(), session.port);
                let stream = crate::transport::build_vless_outbound_transport_over_stream(
                    stream,
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    proxy.config.source_dir(),
                    server,
                    port,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_relay_final_transport",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;
                let (upstream, recv_tx) =
                    establish_vless_udp_upstream_over_stream(proxy, session, id, payload, stream)
                        .await
                        .map_err(|error| FlowFailure {
                            stage: "udp_vless_relay_chain",
                            error,
                            upstream: None,
                        })?;
                self.vless_manager.insert_upstream(key, upstream, recv_tx);
                self.vless_manager.spawn_bridge(
                    &mut self.chain_tasks,
                    session.target.clone(),
                    session.port,
                    session_id,
                );

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send_relay(
                        &mut self.chain_tasks,
                        session.id,
                        stream,
                        None,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: true,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .mieru_manager
                    .send_relay(
                        &mut self.chain_tasks,
                        session.id,
                        stream,
                        server,
                        port,
                        username,
                        password,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
                        relay_chain: true,
                    },
                    tx_bytes: sent as u64,
                })
            }
            _ => Err(FlowFailure {
                stage: "udp_relay_final_hop",
                error: zero_core::Error::Unsupported(
                    "UDP relay chain final hop does not support stream packet UDP",
                )
                .into(),
                upstream: None,
            }),
        }
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
}
