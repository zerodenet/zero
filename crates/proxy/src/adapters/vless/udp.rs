use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;
use manager::{
    model::{VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow},
    VlessUdpOutboundManager,
};

mod manager;

fn parse_vless_udp_identity(
    id: &str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vless::VlessUdpIdentity, FlowFailure> {
    vless::parse_udp_identity(id).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VLESS UDP identity: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

impl VlessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
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
        let tag_owned = (*tag).to_string();
        let identity =
            parse_vless_udp_identity(id, "udp_vless_parse_identity", Some((server, *port)))?;

        let transport = crate::transport::VlessUdpTransportOptions {
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            split_http: *split_http,
            quic: *quic,
            source_dir: proxy.config.source_dir(),
        };
        let mut manager = VlessUdpOutboundManager::new();
        manager
            .start_flow(
                dispatch.protocol_udp_chain_tasks(),
                VlessUdpStartFlow {
                    proxy,
                    session,
                    server,
                    port: *port,
                    identity,
                    flow: *flow,
                    transport,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_upstream",
                error,
                upstream: Some((server.to_string(), *port)),
            })?;
        dispatch.register_cached_protocol_flow_sender(Box::new(manager));

        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Cached {
                tag: tag_owned,
                server: (*server).to_string(),
                port: *port,
            }),
            tx_bytes: 0,
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
            server,
            port,
            id,
            split_http,
            ..
        } = &final_hop
        else {
            return Err(unreachable_udp_leaf(self.name(), &final_hop));
        };
        let identity =
            parse_vless_udp_identity(id, "udp_vless_relay_two_stream_parse_identity", None)?;
        let split_http_cfg = split_http
            .as_ref()
            .expect("udp_relay_needs_two_streams checked split_http is Some");
        let mut manager = VlessUdpOutboundManager::new();
        manager
            .start_relay_two_stream(
                dispatch.protocol_udp_chain_tasks(),
                VlessUdpRelayTwoStream {
                    proxy,
                    session,
                    post_carrier,
                    get_carrier,
                    identity,
                    split_http: split_http_cfg,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        dispatch.register_cached_protocol_flow_sender(Box::new(manager));

        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Cached {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
            }),
            tx_bytes: 0,
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
        let ResolvedLeafOutbound::Vless {
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
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
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
        let identity =
            parse_vless_udp_identity(id, "udp_vless_relay_final_hop_parse_identity", None)?;
        let transport = crate::transport::VlessUdpTransportOptions {
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            split_http: *split_http,
            quic: None,
            source_dir: proxy.config.source_dir(),
        };
        let mut manager = VlessUdpOutboundManager::new();
        manager
            .start_relay_final_hop(
                dispatch.protocol_udp_chain_tasks(),
                VlessUdpRelayFinalHopStart {
                    proxy,
                    session,
                    carrier,
                    identity,
                    transport,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_relay_chain",
                error,
                upstream: None,
            })?;
        dispatch.register_cached_protocol_flow_sender(Box::new(manager));

        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Cached {
                tag: tag_owned,
                server: (*server).to_string(),
                port: *port,
            }),
            tx_bytes: 0,
        })
    }
}
