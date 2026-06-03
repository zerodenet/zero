//! Generic UDP dispatch — protocol-agnostic routing and outbound dispatch.
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
use zero_engine::{EngineError, ResolvedLeafOutbound, ResolvedOutbound, SessionHandle, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::udp_associate::sessions::{
    CompletedUdpFlow, UdpFlowOutbound, UdpFlowSnapshot, UdpSessionFlows,
};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::vless_udp::{VlessUdpOutboundManager, VlessUdpTransport};
use crate::runtime::Proxy;

// Re-export for inbound handlers.
pub(crate) use crate::runtime::udp_helpers::UdpChainResponse;

// ── Chain response types ────────────────────────────────────────────

/// A response item produced by a chain-outbound recv bridge task.
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`UdpDispatch::poll_chain_response`].
type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

// ── SS chain manager ─────────────────────────────────────────────────

/// Per-dispatcher manager for Shadowsocks chain outbound.
///
/// Caches upstream UDP sockets per-dispatcher (not globally) and spawns
/// one-shot bridge tasks into the shared [`JoinSet`] so responses are
/// polled uniformly via [`UdpDispatch::poll_chain_response`].
#[cfg(feature = "shadowsocks")]
mod ss_manager {
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use tokio::sync::broadcast;
    use tokio::task::JoinSet;
    use zero_core::{Address, EngineError};

    use super::{ChainTask, FlowFailure};

    type SsRecvItem = (Address, u16, Vec<u8>);

    struct SsUpstream {
        socket: Arc<tokio::net::UdpSocket>,
        recv_tx: broadcast::Sender<SsRecvItem>,
    }

    pub(super) struct SsChainManager {
        upstreams: HashMap<(String, u16, String, String), Arc<SsUpstream>>,
    }

    impl SsChainManager {
        pub(super) fn new() -> Self {
            Self { upstreams: HashMap::new() }
        }

        pub(super) async fn send(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            server: &str,
            port: u16,
            password: &str,
            cipher: &str,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            use zero_protocol_shadowsocks::{
                aead_encrypt_udp, build_target_data, derive_key, CipherKind,
            };

            let cipher_kind = CipherKind::from_str(cipher).ok_or_else(|| FlowFailure {
                stage: "ss_cipher",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown shadowsocks cipher: {cipher}"),
                )),
                upstream: Some((server.to_owned(), port)),
            })?;

            let entry = self.ensure_entry(server, port, password, cipher_kind);

            let target_data = build_target_data(target, target_port, payload).map_err(|e| {
                FlowFailure {
                    stage: "ss_build_target",
                    error: EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
                    upstream: Some((server.to_owned(), port)),
                }
            })?;

            let mut salt = vec![0u8; cipher_kind.salt_len()];
            use rand::RngCore;
            rand::rngs::OsRng.fill_bytes(&mut salt);

            let key =
                derive_key(password.as_bytes(), &salt, cipher_kind.key_len()).map_err(|e| {
                    FlowFailure {
                        stage: "ss_derive_key",
                        error: EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)),
                        upstream: Some((server.to_owned(), port)),
                    }
                })?;

            let nonce = [0u8; 12];
            let encrypted =
                aead_encrypt_udp(cipher_kind, &key, &nonce, &target_data).map_err(|e| {
                    FlowFailure {
                        stage: "ss_encrypt",
                        error: EngineError::Io(std::io::Error::other(e)),
                        upstream: Some((server.to_owned(), port)),
                    }
                })?;

            let mut packet = salt;
            packet.extend_from_slice(&encrypted);

            let target_addr: SocketAddr = format!("{server}:{port}").parse().map_err(|_| {
                FlowFailure {
                    stage: "ss_parse_addr",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid ss upstream: {server}:{port}"),
                    )),
                    upstream: Some((server.to_owned(), port)),
                }
            })?;

            entry.socket.send_to(&packet, target_addr).await.map_err(|e| FlowFailure {
                stage: "ss_send",
                error: EngineError::from(e),
                upstream: Some((server.to_owned(), port)),
            })?;

            // Spawn one-shot bridge task.
            let mut recv_rx = entry.recv_tx.subscribe();
            chain_tasks.spawn(async move {
                match recv_rx.recv().await {
                    Ok((resp_target, resp_port, resp_payload)) => {
                        Ok((resp_target, resp_port, resp_payload, Some(session_id)))
                    }
                    Err(broadcast::error::RecvError::Closed) => Err(EngineError::Io(
                        std::io::Error::other("ss upstream closed"),
                    )),
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Skip lagged messages and try again
                        match recv_rx.recv().await {
                            Ok((resp_target, resp_port, resp_payload)) => {
                                Ok((resp_target, resp_port, resp_payload, Some(session_id)))
                            }
                            Err(_) => Err(EngineError::Io(std::io::Error::other("ss upstream closed"))),
                        }
                    }
                }
            });

            Ok(payload.len())
        }

        fn ensure_entry(
            &mut self,
            server: &str,
            port: u16,
            password: &str,
            cipher_kind: zero_protocol_shadowsocks::CipherKind,
        ) -> Arc<SsUpstream> {
            use zero_protocol_shadowsocks::CipherKind;
            let key = (
                server.to_owned(), port, cipher_kind.to_string(), password.to_owned(),
            );
            if let Some(entry) = self.upstreams.get(&key) {
                return entry.clone();
            }

            let socket = Arc::new(
                tokio::net::UdpSocket::from_std(
                    std::net::UdpSocket::bind("0.0.0.0:0").expect("ss: bind"),
                )
                .expect("ss: tokio"),
            );

            let (recv_tx, _) = broadcast::channel::<SsRecvItem>(32);
            let entry = Arc::new(SsUpstream { socket: socket.clone(), recv_tx: recv_tx.clone() });
            self.upstreams.insert(key, entry.clone());

            tokio::spawn(Self::recv_loop(socket, cipher_kind, password.to_owned(), recv_tx));
            entry
        }

        async fn recv_loop(
            socket: Arc<tokio::net::UdpSocket>,
            cipher: zero_protocol_shadowsocks::CipherKind,
            password: String,
            recv_tx: broadcast::Sender<SsRecvItem>,
        ) {
            use zero_protocol_shadowsocks::{aead_decrypt_udp, derive_key, parse_target_data};
            let mut buf = vec![0u8; 4096];
            loop {
                let (n, _) = match socket.recv_from(&mut buf).await {
                    Ok(r) => r,
                    Err(_) => break,
                };
                let packet = &buf[..n];
                let sl = cipher.salt_len();
                let tl = cipher.tag_len();
                if packet.len() < sl + tl { continue; }
                let Ok(key) = derive_key(password.as_bytes(), &packet[..sl], cipher.key_len())
                    else { continue };
                let Ok(plain) = aead_decrypt_udp(cipher, &key, &[0u8; 12], &packet[sl..])
                    else { continue };
                let Ok((t, p, off)) = parse_target_data(&plain) else { continue };
                if recv_tx.send((t, p, plain[off..].to_vec())).is_err() { break; }
            }
        }
    }
}
#[cfg(not(feature = "shadowsocks"))]
mod ss_manager {
    use super::{ChainTask, FlowFailure};
    use tokio::task::JoinSet;
    use zero_core::Address;
    pub(super) struct SsChainManager;
    impl SsChainManager {
        pub(super) fn new() -> Self { Self }
        #[allow(unused_variables)]
        pub(super) async fn send(
            &mut self, _tasks: &mut JoinSet<ChainTask>, _session_id: u64,
            _server: &str, _port: u16, _password: &str, _cipher: &str,
            _target: &Address, _target_port: u16, _payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            Err(FlowFailure {
                stage: "ss_feature",
                error: zero_engine::EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Shadowsocks requires feature `shadowsocks`",
                )),
                upstream: None,
            })
        }
    }
}
use ss_manager::SsChainManager;

// ── Types ─────────────────────────────────────────────────────────────

/// Result of starting a new UDP flow.
enum FlowStartResult {
    /// A new flow was established and tracked in `UdpSessionFlows`.
    Flow {
        outbound: UdpFlowOutbound,
        tx_bytes: u64,
    },
    /// A VLESS chain flow was established (tracked by the manager, not `UdpSessionFlows`).
    VlessFlow {
        session_id: u64,
        tag: String,
    },
    /// The target was blocked.
    Blocked {
        tag: String,
    },
}

/// Failure details for a flow start attempt.
struct FlowFailure {
    stage: &'static str,
    error: EngineError,
    upstream: Option<(String, u16)>,
}

// ── UdpDispatch ───────────────────────────────────────────────────────

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
    ss_manager: SsChainManager,
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
            ss_manager: SsChainManager::new(),
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
            ss_manager: SsChainManager::new(),
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────

    /// The direct outbound socket. Inbound handlers poll this for direct
    /// responses and use [`direct_response_session_id`] for metering.
    #[allow(dead_code)]
    pub(crate) fn direct_socket(&self) -> &TokioDatagramSocket {
        &self.direct_socket
    }

    /// Borrow the direct socket and VLESS manager simultaneously.
    ///
    /// This avoids borrow-checker conflicts in `select!` loops where both
    /// the direct socket and VLESS manager are polled concurrently.
    /// Returns chain_tasks for unified chain response polling.
    pub(crate) fn direct_socket_and_vless_manager(
        &mut self,
    ) -> (&TokioDatagramSocket, &mut VlessUdpOutboundManager, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, &mut self.vless_manager, &mut self.chain_tasks)
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    ///
    /// Returns:
    /// - `&TokioDatagramSocket` — direct outbound response socket
    /// - `&mut VlessUdpOutboundManager` — VLESS chain response poller
    /// - `Option<&ActiveUpstreamSocks5UdpAssociation>` — SOCKS5 chain upstream
    /// - `Option<TokioInstant>` — SOCKS5 idle deadline
    /// - `&mut JoinSet<ChainTask>` — SS/H2/VLESS chain response tasks
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        &mut VlessUdpOutboundManager,
        Option<&crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            &self.direct_socket,
            &mut self.vless_manager,
            self.socks5_upstream.as_ref(),
            self.socks5_idle_deadline,
            &mut self.chain_tasks,
        )
    }

    /// Mutable access to the VLESS upstream manager for response polling.
    #[allow(dead_code)]
    pub(crate) fn vless_manager_mut(&mut self) -> &mut VlessUdpOutboundManager {
        &mut self.vless_manager
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
        use crate::outbound::socks5::UpstreamAssociationCloseReason;
        use crate::logging::log_udp_upstream_association_idle_timeout;

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

    /// Drain all pending chain-outbound responses (SS, H2, Trojan, Mieru).
    ///
    /// Returns `(target, port, payload)` tuples for the inbound handler to
    /// encode in protocol-specific format.
    /// Poll the unified chain-outbound response `JoinSet`.
    ///
    /// All chain recv bridge tasks (SS, H2, VLESS, Trojan, Mieru)
    /// are spawned into this set.  Use in `select!` loops alongside
    /// direct-socket and SOCKS5-upstream polls.
    pub(crate) fn poll_chain_response(
        &mut self,
    ) -> &mut JoinSet<ChainTask> {
        &mut self.chain_tasks
    }

    /// Drain pending chain responses from global queues (Trojan/Mieru fallback).
    ///
    /// New SS/H2/VLESS responses flow through [`poll_chain_response`].
    /// Trojan and Mieru still use global queues until their per-dispatcher
    /// managers are implemented.
    pub(crate) fn drain_chain_responses() -> Vec<UdpChainResponse> {
        #[allow(unused_mut)]
        let mut responses = Vec::new();

        #[cfg(feature = "shadowsocks")]
        responses.extend(
            crate::outbound::shadowsocks::drain_all_responses()
                .into_iter()
                .map(|r| UdpChainResponse {
                    target: r.target,
                    port: r.port,
                    payload: r.payload,
                }),
        );

        #[cfg(feature = "hysteria2")]
        responses.extend(
            crate::outbound::hysteria2::drain_all_h2_responses()
                .into_iter()
                .map(|r| UdpChainResponse {
                    target: r.target,
                    port: r.port,
                    payload: r.payload,
                }),
        );

        #[cfg(feature = "trojan")]
        responses.extend(
            crate::outbound::trojan::drain_all_trojan_responses()
                .into_iter()
                .map(|r| UdpChainResponse {
                    target: r.target,
                    port: r.port,
                    payload: r.payload,
                }),
        );

        #[cfg(feature = "mieru")]
        for resp in crate::outbound::mieru_udp::drain_all_mieru_responses() {
            // Mieru responses contain SOCKS5-framed UDP packets.
            if let Ok(parsed) = zero_protocol_socks5::parse_udp_packet(&resp.payload) {
                responses.push(UdpChainResponse {
                    target: parsed.target,
                    port: parsed.port,
                    payload: parsed.payload,
                });
            }
        }

        responses
    }

    // ── Dispatch ──────────────────────────────────────────────────────

    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port)` (including VLESS chain
    /// connections cached in the manager), forwards the payload.  Otherwise
    /// creates a new session, routes through the engine, and dispatches to
    /// the resolved outbound.
    ///
    /// Supports all outbound types: direct, block, socks5, vless,
    /// shadowsocks, hysteria2, trojan, mieru.
    pub(crate) async fn dispatch(
        &mut self,
        proxy: &Proxy,
        target: Address,
        port: u16,
        payload: &[u8],
        protocol: ProtocolType,
        auth: Option<&SessionAuth>,
    ) -> Result<(), EngineError> {
        // ── VLESS manager shortcut (cached upstream) ──────────────
        if let Some(handle) = self.vless_manager.get(&target, port) {
            proxy.record_session_inbound_rx(handle.session_id, payload.len() as u64);
            let _ = handle.send_tx.send(payload.to_vec()).await;
            proxy.record_session_outbound_tx(handle.session_id, payload.len() as u64);
            return Ok(());
        }

        // ── Existing flow (direct / socks5 / ss / h2 / trojan / mieru) ─
        if let Some(flow) = self.flows.snapshot(&target, port) {
            return self.forward_existing(proxy, &flow, payload).await;
        }

        // ── New flow ──────────────────────────────────────────────
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
            ResolvedOutbound::Single(c) => vec![c],
            ResolvedOutbound::Fallback { candidates } => candidates,
            ResolvedOutbound::Relay { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "relay chain not supported for UDP flows",
                )));
            }
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
                    return Ok(());
                }
                Ok(FlowStartResult::VlessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vless_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                    return Ok(());
                }
                Ok(FlowStartResult::Blocked { tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                        log_session_finished(&record, None);
                    }
                    return Ok(());
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
        } else {
            let error = EngineError::Io(std::io::Error::other("all fallback outbounds failed"));
            log_session_failed(
                &session,
                record.as_ref(),
                "fallback_exhausted",
                started_at.elapsed(),
                &error,
                None,
            );
        }

        Ok(())
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
            } => {
                match self.ss_manager.send(
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
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow, started_at, failure.stage, &failure.error.to_string(),
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
                use crate::outbound::hysteria2::send_h2_udp_packet;
                match send_h2_udp_packet(
                    proxy,
                    &flow.session,
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
                    Err(error) => {
                        let msg = error.to_string();
                        self.fail_flow_with_msg(&flow, started_at, "udp_h2_send", &msg);
                        return Err(EngineError::Io(std::io::Error::other(msg)));
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
            } => {
                use crate::outbound::trojan::send_trojan_udp_packet;
                match send_trojan_udp_packet(
                    proxy,
                    &flow.session,
                    server.as_str(),
                    *port,
                    password.as_str(),
                    sni.as_deref(),
                    *insecure,
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
                    Err(error) => {
                        let msg = error.to_string();
                        self.fail_flow_with_msg(&flow, started_at, "udp_trojan_send", &msg);
                        return Err(EngineError::Io(std::io::Error::other(msg)));
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
            } => {
                use crate::outbound::mieru_udp::send_mieru_udp_packet;
                match send_mieru_udp_packet(
                    proxy,
                    &flow.session,
                    server.as_str(),
                    *port,
                    username.as_str(),
                    password.as_str(),
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        let msg = error.to_string();
                        self.fail_flow_with_msg(&flow, started_at, "udp_mieru_send", &msg);
                        return Err(EngineError::Io(std::io::Error::other(msg)));
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
        candidate: ResolvedLeafOutbound<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
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
                        proxy,
                        tag,
                        server,
                        port,
                        username,
                        password,
                        session,
                        payload,
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
                let sent = crate::outbound::hysteria2::send_h2_udp_packet(
                    proxy,
                    session,
                    server,
                    port,
                    password,
                    client_fingerprint,
                    &session.target,
                    session.port,
                    payload,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_h2_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
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
                    let sent = self.ss_manager.send(
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
                let sent = crate::outbound::trojan::send_trojan_udp_packet(
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
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_trojan_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
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
                let sent = crate::outbound::mieru_udp::send_mieru_udp_packet(
                    proxy,
                    session,
                    server,
                    port,
                    username,
                    password,
                    &session.target,
                    session.port,
                    payload,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_mieru_send",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
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

    // ── SOCKS5 helper ─────────────────────────────────────────────────

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
        use crate::outbound::socks5::{
            send_socks5_udp_packet, Socks5UdpAssociation, UpstreamAssociationCloseReason,
        };
        use crate::logging::log_udp_upstream_association_dropped;

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
                proxy.record_udp_upstream_packet_sent();
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

    // ── Failure helpers ───────────────────────────────────────────────

    fn fail_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        error: &EngineError,
    ) {
        if let Some(completed) =
            self.flows
                .finish(&flow.session.target, flow.session.port, SessionOutcome::Failed)
        {
            log_session_failed(
                &flow.session,
                Some(&completed.record),
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        } else {
            log_session_failed(&flow.session, None, stage, started_at.elapsed(), error, None);
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
