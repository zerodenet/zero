pub(crate) use crate::protocol_runtime::vless_udp::model::{
    VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream,
    VlessUdpStartFlow,
};
use crate::protocol_runtime::vless_udp::VlessUdpOutboundManager;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

pub(crate) async fn send_datagram(
    dispatch: &mut UdpDispatch,
    request: VlessUdpFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    let transport = crate::transport::VlessUdpTransportOptions {
        tls: request.tls,
        reality: request.reality,
        ws: request.ws,
        grpc: request.grpc,
        h2: request.h2,
        http_upgrade: request.http_upgrade,
        split_http: request.split_http,
        quic: request.quic,
        source_dir: request.proxy.config.source_dir(),
    };
    let mut manager = VlessUdpOutboundManager::new();
    manager
        .start_flow(
            chain_tasks,
            VlessUdpStartFlow {
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                identity: request.identity,
                flow: request.flow,
                transport,
                payload: request.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_upstream",
            error,
            upstream: Some((request.server.to_string(), request.port)),
        })?;
    protocol_state.register_cached_flow_sender(Box::new(manager));
    Ok(())
}

pub(crate) async fn send_relay_two_stream(
    dispatch: &mut UdpDispatch,
    request: VlessUdpRelayTwoStream<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    let mut manager = VlessUdpOutboundManager::new();
    manager
        .start_relay_two_stream(
            chain_tasks,
            VlessUdpRelayTwoStream {
                proxy: request.proxy,
                session: request.session,
                post_carrier: request.post_carrier,
                get_carrier: request.get_carrier,
                identity: request.identity,
                split_http: request.split_http,
                payload: request.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_relay_chain",
            error,
            upstream: None,
        })?;
    protocol_state.register_cached_flow_sender(Box::new(manager));
    Ok(())
}

pub(crate) async fn send_relay_final_hop(
    dispatch: &mut UdpDispatch,
    request: VlessUdpRelayFinalHop<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    let transport = crate::transport::VlessUdpTransportOptions {
        tls: request.tls,
        reality: request.reality,
        ws: request.ws,
        grpc: request.grpc,
        h2: request.h2,
        http_upgrade: request.http_upgrade,
        split_http: request.split_http,
        quic: None,
        source_dir: request.proxy.config.source_dir(),
    };
    let mut manager = VlessUdpOutboundManager::new();
    manager
        .start_relay_final_hop(
            chain_tasks,
            VlessUdpRelayFinalHopStart {
                proxy: request.proxy,
                session: request.session,
                carrier: request.carrier,
                identity: request.identity,
                transport,
                payload: request.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_relay_chain",
            error,
            upstream: None,
        })?;
    protocol_state.register_cached_flow_sender(Box::new(manager));
    Ok(())
}
