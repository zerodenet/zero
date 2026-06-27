use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use super::managed::{VmessUdpRelayFlowStart, VmessUdpStartFlow};
use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedStreamPacketSender;
use crate::runtime::Proxy;

fn vmess_udp_flow_config<'a>(
    id: &str,
    cipher: &'a str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vmess::VmessUdpFlowConfig<'a>, FlowFailure> {
    vmess::udp_flow_config_from_config(id, cipher).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VMess UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
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
    let config =
        vmess_udp_flow_config(id, cipher, "udp_vmess_parse_config", Some((server, *port)))?;
    let transport = crate::transport::VmessTransportOptions {
        tls: *tls,
        ws: *ws,
        grpc: *grpc,
        source_dir: proxy.config.source_dir(),
    };
    let mut sender = ManagedStreamPacketSender::new();
    super::managed::start_flow(
        &mut sender,
        dispatch.managed_udp_chain_tasks(),
        VmessUdpStartFlow {
            proxy,
            mux_pool: &adapter.mux_pool,
            session,
            server,
            port: *port,
            config,
            mux_concurrency: *mux_concurrency,
            transport,
            payload,
        },
    )
    .await
    .map_err(|error| FlowFailure {
        stage: "udp_vmess_upstream",
        error,
        upstream: Some((server.to_string(), *port)),
    })?;
    Ok(dispatch.register_managed_stream_packet_flow(tag, server, *port, Box::new(sender)))
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
    let config = vmess_udp_flow_config(
        id,
        cipher,
        "udp_vmess_relay_final_hop_parse_config",
        Some((server, *port)),
    )?;
    let transport = crate::transport::VmessTransportOptions {
        tls: *tls,
        ws: *ws,
        grpc: *grpc,
        source_dir: proxy.config.source_dir(),
    };
    let mut sender = ManagedStreamPacketSender::new();
    super::managed::start_relay_flow(
        &mut sender,
        dispatch.managed_udp_chain_tasks(),
        VmessUdpRelayFlowStart {
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
        stage: "udp_vmess_relay_chain",
        error,
        upstream: None,
    })?;
    Ok(dispatch.register_managed_stream_packet_flow(tag, server, *port, Box::new(sender)))
}
