use std::any::Any;
use std::future::Future;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::super::super::error::relay_chain_flow_failure;
use super::super::super::stream_packet::start_relay_managed_stream_packet;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;
use crate::transport::{RelayCarrier, TcpRelayStream};

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_relay_two_stream_managed_flow<T, FBuild, FBuildFut>(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    chain: &[ResolvedLeafOutbound<'_>],
    tag: &str,
    server: &str,
    port: u16,
    paired_stage: &'static str,
    build_transport: FBuild,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
    FBuild: FnOnce(TcpRelayStream, TcpRelayStream) -> FBuildFut,
    FBuildFut: Future<Output = Result<TcpRelayStream, EngineError>>,
{
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
        dispatch,
        Some(proxy),
        tag,
        session,
        RelayCarrier {
            stream: paired_stream,
            server: server.to_string(),
            port,
        },
        None,
        server,
        port,
        resume,
        payload,
    )
    .await
}
