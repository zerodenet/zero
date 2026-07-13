use std::any::Any;
use std::future::Future;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::super::super::error::relay_chain_flow_failure;
use super::super::super::stream_packet::{
    start_relay_managed_stream_packet, ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};
use crate::runtime::udp_flow::state::UdpFlowStartContext;
use crate::runtime::Proxy;
use crate::transport::{RelayCarrier, TcpRelayStream};

pub(super) struct RelayTwoStreamManagedFlowRequest<'a, 'chain, 'leaf, T> {
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) chain: &'chain [ResolvedLeafOutbound<'leaf>],
    pub(super) tag: &'a str,
    pub(super) endpoint: (&'a str, u16),
    pub(super) paired_stage: &'static str,
    pub(super) resume: T,
    pub(super) payload: &'a [u8],
}

pub(super) async fn start_relay_two_stream_managed_flow<T, FBuild, FBuildFut>(
    context: &mut UdpFlowStartContext<'_>,
    request: RelayTwoStreamManagedFlowRequest<'_, '_, '_, T>,
    build_transport: FBuild,
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
    FBuild: FnOnce(TcpRelayStream, TcpRelayStream) -> FBuildFut,
    FBuildFut: Future<Output = Result<TcpRelayStream, EngineError>>,
{
    let RelayTwoStreamManagedFlowRequest {
        proxy,
        session,
        chain,
        tag,
        endpoint: (server, port),
        paired_stage,
        resume,
        payload,
    } = request;
    let chain_post = chain.to_vec();
    let chain_get = chain.to_vec();
    let (post_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_post)
        .await
        .map_err(relay_chain_flow_failure)?;
    let (get_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_get)
        .await
        .map_err(relay_chain_flow_failure)?;
    let paired_stream = build_transport(post_carrier.stream, get_carrier.stream)
        .await
        .map_err(|error| FlowFailure {
            stage: paired_stage,
            error,
            upstream: None,
        })?;

    start_relay_managed_stream_packet(
        context,
        ManagedStreamPacketStartBridge::relay(
            Some(proxy),
            tag,
            session,
            ManagedStreamPacketRelay {
                carrier: RelayCarrier {
                    stream: paired_stream,
                    server: server.to_string(),
                    port,
                },
                tls_server_name: None,
            },
            (server, port),
            resume,
            payload,
        ),
    )
    .await
}
