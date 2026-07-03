use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
use crate::runtime::Proxy;

fn invalid_vmess_udp_config(
    error: impl std::fmt::Display,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VMess UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    }
}

pub(super) async fn start(
    adapter: &VmessAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Vmess {
        tag,
        server,
        port,
        id,
        cipher,
        mux_concurrency,
        mux_idle_timeout_secs: _,
        tls,
        ws,
        grpc,
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let resume = vmess::udp::udp_flow_resume_from_config(id, cipher, false).map_err(|error| {
        invalid_vmess_udp_config(error, "udp_vmess_parse_config", Some((server, *port)))
    })?;
    let transport = crate::transport::VmessTransportOptions {
        tls: *tls,
        ws: *ws,
        grpc: *grpc,
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
            resume: super::managed::resume(adapter, resume, *mux_concurrency, transport),
            payload,
            relay_chain: false,
        })
        .await
}

pub(super) async fn start_relay_final_hop(
    adapter: &VmessAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Vmess {
        tag,
        server,
        port,
        id,
        cipher,
        tls,
        ws,
        grpc,
        ..
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let resume = vmess::udp::udp_flow_resume_from_config(id, cipher, true).map_err(|error| {
        invalid_vmess_udp_config(
            error,
            "udp_vmess_relay_final_hop_parse_config",
            Some((server, *port)),
        )
    })?;
    let transport = crate::transport::VmessTransportOptions {
        tls: *tls,
        ws: *ws,
        grpc: *grpc,
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
            resume: super::managed::resume(adapter, resume, None, transport),
            payload,
            relay_chain: true,
        })
        .await
}
