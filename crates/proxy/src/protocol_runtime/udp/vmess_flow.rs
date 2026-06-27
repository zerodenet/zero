pub(crate) use crate::protocol_runtime::vmess_udp::model::{
    VmessUdpFlow, VmessUdpRelayFlow, VmessUdpRelayFlowStart, VmessUdpStartFlow,
};
use crate::protocol_runtime::vmess_udp::VmessUdpOutboundManager;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};

pub(crate) async fn send_datagram(
    dispatch: &mut UdpDispatch,
    request: VmessUdpFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    let transport = crate::transport::VmessTransportOptions {
        tls: request.tls,
        ws: request.ws,
        grpc: request.grpc,
        source_dir: request.proxy.config.source_dir(),
    };
    let mut manager = VmessUdpOutboundManager::new();
    manager
        .start_flow(
            chain_tasks,
            VmessUdpStartFlow {
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                identity: request.identity,
                cipher_name: request.cipher_name,
                mux_concurrency: request.mux_concurrency,
                transport,
                payload: request.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vmess_upstream",
            error,
            upstream: Some((request.server.to_string(), request.port)),
        })?;
    protocol_state.register_cached_flow_sender(Box::new(manager));
    Ok(())
}

pub(crate) async fn send_relay(
    dispatch: &mut UdpDispatch,
    request: VmessUdpRelayFlow<'_>,
) -> Result<(), FlowFailure> {
    let (protocol_state, chain_tasks) = dispatch.protocol_udp_state_and_chain_tasks();
    let transport = crate::transport::VmessTransportOptions {
        tls: request.tls,
        ws: request.ws,
        grpc: request.grpc,
        source_dir: request.proxy.config.source_dir(),
    };
    let mut manager = VmessUdpOutboundManager::new();
    manager
        .start_relay_flow(
            chain_tasks,
            VmessUdpRelayFlowStart {
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
            stage: "udp_vmess_relay_chain",
            error,
            upstream: None,
        })?;
    protocol_state.register_cached_flow_sender(Box::new(manager));
    Ok(())
}
