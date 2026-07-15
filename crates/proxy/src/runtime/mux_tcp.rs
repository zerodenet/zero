use crate::runtime::route_runtime::MuxSubstreamRuntime;
use tracing::warn;
use zero_core::{InboundMuxTcpRelay, Session};

pub(crate) struct MuxTcpStreamTask<B> {
    pub(crate) session: Session,
    pub(crate) bridge: B,
    pub(crate) protocol: &'static str,
}

pub(crate) async fn run_mux_tcp_stream_task<B>(
    runtime: MuxSubstreamRuntime,
    request: MuxTcpStreamTask<B>,
) where
    B: InboundMuxTcpRelay,
{
    let MuxTcpStreamTask {
        mut session,
        bridge,
        protocol,
    } = request;
    let mux_session_id = bridge.mux_session_id();

    let upstream = match runtime.open_tcp_upstream(&mut session).await {
        Ok(result) => result.upstream,
        Err(error) => {
            warn!(%error, mux_session_id, protocol, "mux tcp dispatch failed");
            bridge.close_stream().await;
            return;
        }
    };

    bridge.relay_stream(upstream).await;
}

pub(crate) async fn run_protocol_mux_tcp_task<B>(
    runtime: MuxSubstreamRuntime,
    session: Session,
    bridge: B,
    protocol: &'static str,
) where
    B: InboundMuxTcpRelay,
{
    run_mux_tcp_stream_task(
        runtime,
        MuxTcpStreamTask {
            session,
            bridge,
            protocol,
        },
    )
    .await;
}
