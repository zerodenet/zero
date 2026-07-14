use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound, ResolvedOutbound};

use super::super::ProtocolInventory;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use crate::runtime::Proxy;

enum UdpCandidate<'a> {
    Leaf(ResolvedLeafOutbound<'a>),
    Relay(Vec<ResolvedLeafOutbound<'a>>),
}

impl ProtocolInventory {
    pub(crate) async fn start_udp_resolved_outbound(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        resolved: ResolvedOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidates = match resolved {
            ResolvedOutbound::Single(candidate) => vec![UdpCandidate::Leaf(candidate)],
            ResolvedOutbound::Fallback { candidates } => {
                candidates.into_iter().map(UdpCandidate::Leaf).collect()
            }
            ResolvedOutbound::Relay { chain } => vec![UdpCandidate::Relay(chain)],
        };
        let is_fallback = candidates.len() > 1;
        let mut last_failure = None;

        for candidate in candidates {
            match self
                .start_udp_candidate(dispatch, proxy, session, candidate, payload)
                .await
            {
                Ok(result) => return Ok(result),
                Err(failure) => last_failure = Some(failure),
            }
        }

        Err(last_failure.unwrap_or_else(|| FlowFailure {
            stage: if is_fallback {
                "fallback_exhausted"
            } else {
                "udp_outbound"
            },
            error: EngineError::Io(std::io::Error::other("all fallback outbounds failed")),
            upstream: None,
        }))
    }

    async fn start_udp_candidate(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        candidate: UdpCandidate<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidate = match candidate {
            UdpCandidate::Leaf(candidate) => candidate,
            UdpCandidate::Relay(chain) => {
                return self
                    .start_udp_relay_chain(dispatch, proxy, session, chain, payload)
                    .await;
            }
        };

        let runtime = self
            .outbound_leaf_runtime(&candidate)
            .map_err(|error| FlowFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream: None,
            })?;
        if !proxy.udp_enabled_for_outbound(runtime.udp_policy_tag) {
            return Err(FlowFailure {
                stage: "udp_policy",
                error: EngineError::Io(std::io::Error::other("udp disabled for outbound")),
                upstream: runtime
                    .endpoint
                    .map(|endpoint| (endpoint.server.to_owned(), endpoint.port)),
            });
        }
        if matches!(
            runtime.tcp_path,
            crate::runtime::path::TcpPathCategory::Block
        ) {
            return Ok(FlowStartResult::Blocked {
                tag: runtime.kernel_tag.unwrap_or("block").to_string(),
            });
        }

        self.start_udp_leaf_flow(dispatch, proxy, session, &candidate, payload)
            .await
    }

    async fn start_udp_relay_chain(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        for leaf in &chain {
            let runtime = self
                .outbound_leaf_runtime(leaf)
                .map_err(|error| FlowFailure {
                    stage: "outbound_leaf_runtime",
                    error,
                    upstream: None,
                })?;
            if !proxy.udp_enabled_for_outbound(runtime.udp_policy_tag) {
                return Err(FlowFailure {
                    stage: "udp_policy",
                    error: EngineError::Io(std::io::Error::other(
                        "udp disabled for relay chain outbound",
                    )),
                    upstream: runtime
                        .endpoint
                        .map(|endpoint| (endpoint.server.to_owned(), endpoint.port)),
                });
            }
        }

        if chain.len() == 2 {
            let carrier_leaf = &chain[0];
            let datagram_leaf = &chain[1];
            if let Some((flow_binding, start_request)) = self.prepare_udp_packet_path_pair(
                session.id,
                carrier_leaf,
                datagram_leaf,
                UdpPacketRef {
                    target: &session.target,
                    port: session.port,
                    payload,
                },
            ) {
                let sent = dispatch
                    .send_packet_path_chain(proxy, start_request)
                    .await?;

                return Ok(FlowStartResult::Flow {
                    outbound: Box::new(UdpDispatch::datagram_chain_flow_outbound(flow_binding)),
                    tx_bytes: sent as u64,
                });
            }
        }

        let final_hop = chain.last().expect("relay chain has at least 2 hops");
        if self
            .udp_relay_needs_two_streams(final_hop)
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?
        {
            return self
                .start_udp_relay_two_stream(dispatch, proxy, session, chain, payload)
                .await;
        }

        let (carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
                .await
                .map_err(|failure| FlowFailure {
                    stage: failure.stage,
                    error: failure.error,
                    upstream: failure.upstream_endpoint,
                })?;

        self.start_udp_relay_final_hop(dispatch, proxy, session, carrier, &final_hop, payload)
            .await
    }
}
