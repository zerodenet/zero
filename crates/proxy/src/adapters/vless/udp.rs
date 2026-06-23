use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

impl VlessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::VlessUdpFlow;
        use zero_traits::UdpPacketFraming;

        let ResolvedLeafOutbound::Vless {
            tag,
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
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let session_id = session.id;
        let tag_owned = (*tag).to_string();

        let mux_flow_enabled =
            *flow == Some("xtls-rprx-vision") || *flow == Some("xtls-rprx-vision-udp443");
        if mux_flow_enabled {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = proxy
                .mux_pool
                .open_udp_stream(
                    crate::protocol_runtime::vless_mux_pool::VlessMuxOpenRequest {
                        proxy,
                        session: None,
                        server,
                        port: *port,
                        id: &::vless::parse_uuid(id).map_err(|e| FlowFailure {
                            stage: "udp_vless_mux_parse_uuid",
                            error: EngineError::Io(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                format!("invalid VLESS UUID: {e}"),
                            )),
                            upstream: Some(((*server).to_string(), *port)),
                        })?,
                        tls: *tls,
                        reality: *reality,
                        max_concurrency,
                    },
                )
                .await
            {
                let packet = <::vless::VlessOutbound as UdpPacketFraming<
                    ::vless::VlessUdpPacketTarget,
                >>::encode_udp_packet(
                    &proxy.protocols.vless_outbound_protocol(),
                    &::vless::VlessUdpPacketTarget {
                        address: &session.target,
                        port: session.port,
                        payload,
                    },
                )
                .map_err(|error| FlowFailure {
                    stage: "udp_vless_mux_encode",
                    error: error.into(),
                    upstream: Some(((*server).to_string(), *port)),
                })?;
                let _ = up_tx.send(packet);
                proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                return Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                });
            }
        }

        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        protocol_state
            .start_vless_udp_flow(
                chain_tasks,
                VlessUdpFlow {
                    proxy,
                    session,
                    server,
                    port: *port,
                    id,
                    flow: *flow,
                    tls: *tls,
                    reality: *reality,
                    ws: *ws,
                    grpc: *grpc,
                    h2: *h2,
                    http_upgrade: *http_upgrade,
                    split_http: *split_http,
                    quic: *quic,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: error.stage,
                error: error.error,
                upstream: error.upstream,
            })?;

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: tag_owned,
        })
    }

    pub(super) fn udp_relay_needs_two_streams_impl(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        let _ = self;
        matches!(
            leaf,
            ResolvedLeafOutbound::Vless {
                split_http: Some(cfg),
                ..
            } if !zero_transport::split_http::XhttpMode::parse(&cfg.mode).is_single_connection()
        )
    }

    pub(super) async fn start_udp_relay_two_stream_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::VlessUdpRelayTwoStream;

        let chain_get = chain.clone();
        let (post_carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
                .await
                .map_err(|f| FlowFailure {
                    stage: f.stage,
                    error: f.error,
                    upstream: f.upstream_endpoint,
                })?;
        let (get_carrier, _) = proxy
            .dispatch_tcp_relay_prefix(chain_get)
            .await
            .map_err(|f| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream_endpoint,
            })?;

        let ResolvedLeafOutbound::Vless {
            tag,
            server: _,
            port: _,
            id,
            split_http,
            ..
        } = &final_hop
        else {
            return Err(unreachable_udp_leaf(self.name(), &final_hop));
        };
        let session_id = session.id;
        let split_http_cfg = split_http
            .as_ref()
            .expect("udp_relay_needs_two_streams checked split_http is Some");
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        protocol_state
            .start_vless_udp_relay_two_stream(
                chain_tasks,
                VlessUdpRelayTwoStream {
                    proxy,
                    session,
                    post_carrier,
                    get_carrier,
                    id,
                    split_http: split_http_cfg,
                    payload,
                },
            )
            .await?;

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: (*tag).to_string(),
        })
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::VlessUdpRelayFinalHop;

        let ResolvedLeafOutbound::Vless {
            tag,
            server: _,
            port: _,
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
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let session_id = session.id;
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

        let tag_owned = (*tag).to_string();
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        protocol_state
            .start_vless_udp_relay_final_hop(
                chain_tasks,
                VlessUdpRelayFinalHop {
                    proxy,
                    session,
                    carrier,
                    id,
                    tls: *tls,
                    reality: *reality,
                    ws: *ws,
                    grpc: *grpc,
                    h2: *h2,
                    http_upgrade: *http_upgrade,
                    split_http: *split_http,
                    payload,
                },
            )
            .await?;

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: tag_owned,
        })
    }
}
