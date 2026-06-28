use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use super::managed::{VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow};
use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedStreamPacketSender;
use crate::runtime::Proxy;

fn vless_udp_flow_config<'a>(
    id: &str,
    flow: Option<&'a str>,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vless::udp::VlessUdpFlowConfig<'a>, FlowFailure> {
    vless::udp::udp_flow_config_from_config(id, flow).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VLESS UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

pub(super) async fn start(
    adapter: &VlessAdapter,
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
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let config = vless_udp_flow_config(id, *flow, "udp_vless_parse_config", Some((server, *port)))?;

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
    let mut sender = ManagedStreamPacketSender::new();
    super::managed::start_flow(
        &mut sender,
        dispatch.managed_udp_chain_tasks(),
        VlessUdpStartFlow {
            proxy,
            mux_pool: &adapter.mux_pool,
            session,
            server,
            port: *port,
            config,
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
    Ok(dispatch.register_managed_stream_packet_flow(tag, server, *port, Box::new(sender)))
}

pub(super) async fn start_relay_two_stream(
    adapter: &VlessAdapter,
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
        return Err(unreachable_udp_leaf(adapter.name(), &final_hop));
    };
    let config = vless_udp_flow_config(id, None, "udp_vless_relay_two_stream_parse_config", None)?;
    let split_http_cfg = split_http
        .as_ref()
        .expect("udp_relay_needs_two_streams checked split_http is Some");
    let mut sender = ManagedStreamPacketSender::new();
    super::managed::start_relay_two_stream(
        &mut sender,
        dispatch.managed_udp_chain_tasks(),
        VlessUdpRelayTwoStream {
            proxy,
            session,
            post_carrier,
            get_carrier,
            config,
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
    Ok(dispatch.register_managed_stream_packet_flow(tag, server, *port, Box::new(sender)))
}

pub(super) async fn start_relay_final_hop(
    adapter: &VlessAdapter,
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
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
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

    let config = vless_udp_flow_config(id, None, "udp_vless_relay_final_hop_parse_config", None)?;
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
    let mut sender = ManagedStreamPacketSender::new();
    super::managed::start_relay_final_hop(
        &mut sender,
        dispatch.managed_udp_chain_tasks(),
        VlessUdpRelayFinalHopStart {
            proxy,
            session,
            carrier,
            config,
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
    Ok(dispatch.register_managed_stream_packet_flow(tag, server, *port, Box::new(sender)))
}
