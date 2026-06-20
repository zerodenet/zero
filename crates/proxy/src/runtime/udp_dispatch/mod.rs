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
use std::time::Instant;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::udp_associate::sessions::{UdpFlowSnapshot, UdpSessionFlows};
use crate::runtime::vless_udp::VlessUdpOutboundManager;
#[cfg(feature = "vmess")]
use crate::runtime::vmess_udp::VmessUdpOutboundManager;
use crate::runtime::Proxy;
use zero_core::{Address, Network, ProtocolType, Session, SessionAuth};
use zero_engine::{EngineError, SessionHandle, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::inbound_protocol::apply_kernel_rate_limits;

// Sub-module declarations.

mod forward;
mod lifecycle;
mod socks5_flow;
mod start;
mod types;

mod packet_path_traits;

mod h2_manager;
mod mieru_manager;
#[cfg(feature = "shadowsocks")]
mod packet_path_chain;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod trojan_manager;

// Re-exports.

#[cfg(all(feature = "shadowsocks", feature = "socks5"))]
pub(crate) use crate::runtime::socks5_udp::build_socks5_packet_path;
use h2_manager::H2ChainManager;
use mieru_manager::MieruChainManager;
#[cfg(all(feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) use packet_path_chain::build_hysteria2_packet_path;
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_chain::build_shadowsocks_packet_path;
#[cfg(feature = "shadowsocks")]
use packet_path_chain::PacketPathManager;
pub(crate) use packet_path_traits::ChainTask;
pub(crate) use packet_path_traits::{
    PacketPathCarrier, PacketPathCarrierDescriptor, UdpDatagramSource,
};
#[cfg(feature = "shadowsocks")]
use ss_manager::SsChainManager;
use trojan_manager::TrojanChainManager;
pub(crate) use types::{FlowFailure, FlowStartResult, UdpCandidate};

// UdpDispatch.

/// Protocol-agnostic UDP dispatch state.
///
/// Owns all outbound-specific state (direct socket, upstream associations,
/// VLESS manager) and session flow tracking.  Created per inbound UDP
/// session/association.
pub(crate) struct UdpDispatch {
    pub(crate) inbound_tag: String,
    pub(crate) flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    pub(crate) direct_socket: TokioDatagramSocket,
    /// SOCKS5 upstream association (shared across all flows in this session).
    pub(crate) socks5_upstream:
        Option<crate::runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>,
    pub(crate) socks5_idle_deadline: Option<TokioInstant>,
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
    pub(crate) chain_tasks: JoinSet<ChainTask>,
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
        if let Some(session_id) = self
            .vless_manager
            .send_existing(&mut self.chain_tasks, proxy, &target, port, payload)
            .await?
        {
            return Ok(session_id);
        }

        #[cfg(feature = "vmess")]
        if let Some(session_id) = self
            .vmess_manager
            .send_existing(&mut self.chain_tasks, proxy, &target, port, payload)
            .await?
        {
            return Ok(session_id);
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
        apply_kernel_rate_limits(proxy, &mut session, &self.inbound_tag);
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

    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn start_shadowsocks_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        cipher: &str,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.ss_manager
            .send_existing(
                &mut self.chain_tasks,
                session.id,
                proxy,
                server,
                port,
                password,
                cipher,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn start_hysteria2_udp_flow(
        &mut self,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.h2_manager
            .send_existing(
                &mut self.chain_tasks,
                session.id,
                server,
                port,
                password,
                client_fingerprint,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "trojan")]
    pub(crate) async fn start_trojan_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.trojan_manager
            .send_existing(
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
                relay_chain,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "trojan")]
    pub(crate) async fn start_trojan_udp_relay_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.trojan_manager
            .send_relay_existing(
                &mut self.chain_tasks,
                session.id,
                carrier.stream,
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
            .await
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "mieru")]
    pub(crate) async fn start_mieru_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.mieru_manager
            .send_existing(
                &mut self.chain_tasks,
                session.id,
                proxy,
                session,
                server,
                port,
                username,
                password,
                relay_chain,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[cfg(feature = "mieru")]
    pub(crate) async fn start_mieru_udp_relay_flow(
        &mut self,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.mieru_manager
            .send_relay_existing(
                &mut self.chain_tasks,
                session.id,
                carrier.stream,
                server,
                port,
                username,
                password,
                &session.target,
                session.port,
                payload,
            )
            .await
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        id: &str,
        flow: Option<&str>,
        tls: Option<&zero_config::ClientTlsConfig>,
        reality: Option<&zero_config::RealityConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
        h2: Option<&zero_config::H2Config>,
        http_upgrade: Option<&zero_config::HttpUpgradeConfig>,
        split_http: Option<&zero_config::SplitHttpConfig>,
        quic: Option<&zero_config::QuicConfig>,
        payload: &[u8],
    ) -> Result<(), FlowFailure> {
        self.vless_manager
            .start_flow(
                &mut self.chain_tasks,
                proxy,
                session,
                server,
                port,
                id,
                flow,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_upstream",
                error,
                upstream: Some((server.to_string(), port)),
            })?;
        Ok(())
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_relay_two_stream(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        post_carrier: crate::transport::RelayCarrier,
        get_carrier: crate::transport::RelayCarrier,
        id: &str,
        split_http: &zero_config::SplitHttpConfig,
        payload: &[u8],
    ) -> Result<(), FlowFailure> {
        self.vless_manager
            .start_relay_two_stream(
                &mut self.chain_tasks,
                proxy,
                session,
                post_carrier,
                get_carrier,
                id,
                split_http,
                payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn start_vless_udp_relay_final_hop(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        id: &str,
        tls: Option<&zero_config::ClientTlsConfig>,
        reality: Option<&zero_config::RealityConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
        h2: Option<&zero_config::H2Config>,
        http_upgrade: Option<&zero_config::HttpUpgradeConfig>,
        split_http: Option<&zero_config::SplitHttpConfig>,
        payload: &[u8],
    ) -> Result<(), FlowFailure> {
        self.vless_manager
            .start_relay_final_hop(
                &mut self.chain_tasks,
                proxy,
                session,
                carrier,
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
                payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn start_vmess_udp_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        tls: Option<&zero_config::ClientTlsConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
        payload: &[u8],
    ) -> Result<(), FlowFailure> {
        self.vmess_manager
            .start_flow(
                &mut self.chain_tasks,
                proxy,
                session,
                server,
                port,
                id,
                cipher,
                mux_concurrency,
                tls,
                ws,
                grpc,
                payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_upstream",
                error,
                upstream: Some((server.to_string(), port)),
            })?;
        Ok(())
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn start_vmess_udp_relay_flow(
        &mut self,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        tls: Option<&zero_config::ClientTlsConfig>,
        ws: Option<&zero_config::WebSocketConfig>,
        grpc: Option<&zero_config::GrpcConfig>,
        payload: &[u8],
    ) -> Result<(), FlowFailure> {
        self.vmess_manager
            .start_relay_flow(
                &mut self.chain_tasks,
                proxy,
                session,
                carrier,
                server,
                port,
                id,
                cipher,
                tls,
                ws,
                grpc,
                payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
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
