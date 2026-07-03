use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
use crate::runtime::Proxy;

fn invalid_vless_udp_config(
    error: impl std::fmt::Display,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VLESS UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    }
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
    let resume = vless::udp::udp_flow_resume_from_config(id, *flow, false).map_err(|error| {
        invalid_vless_udp_config(error, "udp_vless_parse_config", Some((server, *port)))
    })?;
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
    dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: Some(proxy),
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port: *port,
            resume: super::managed::direct_resume(adapter, resume, transport),
            payload,
            relay_chain: false,
        })
        .await
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
    let resume = vless::udp::udp_flow_resume_from_config(id, None, true).map_err(|error| {
        invalid_vless_udp_config(error, "udp_vless_relay_two_stream_parse_config", None)
    })?;
    let split_http_cfg = split_http
        .as_ref()
        .expect("udp_relay_needs_two_streams checked split_http is Some");
    let paired_stream = crate::transport::build_vless_split_http_over_relay(
        post_carrier.stream,
        get_carrier.stream,
        split_http_cfg,
    )
    .await
    .map_err(|error| FlowFailure {
        stage: "udp_vless_relay_chain",
        error,
        upstream: None,
    })?;
    let transport = crate::transport::VlessUdpTransportOptions {
        tls: None,
        reality: None,
        ws: None,
        grpc: None,
        h2: None,
        http_upgrade: None,
        split_http: split_http.as_ref().map(|config| *config),
        quic: None,
        source_dir: proxy.config.source_dir(),
    };
    dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: Some(proxy),
            tag,
            session,
            carrier: Some(crate::transport::RelayCarrier {
                stream: paired_stream,
                server: (*server).to_string(),
                port: *port,
            }),
            tls_server_name: None,
            server,
            port: *port,
            resume: super::managed::relay_paired_transport_resume(adapter, resume, transport),
            payload,
            relay_chain: true,
        })
        .await
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

    let resume = vless::udp::udp_flow_resume_from_config(id, None, true).map_err(|error| {
        invalid_vless_udp_config(error, "udp_vless_relay_final_hop_parse_config", None)
    })?;
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
    dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: Some(proxy),
            tag,
            session,
            carrier: Some(carrier),
            tls_server_name: None,
            server,
            port: *port,
            resume: super::managed::relay_final_hop_resume(adapter, resume, transport),
            payload,
            relay_chain: true,
        })
        .await
}
