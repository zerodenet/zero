use tracing::warn;
use zero_core::{InboundMuxTcpRelay, Session};
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::TcpRouteResult;

pub(crate) struct MuxTcpStreamTask<B> {
    pub(crate) session: Session,
    pub(crate) bridge: B,
    pub(crate) inbound_tag: String,
    pub(crate) protocol: &'static str,
}

pub(crate) async fn run_mux_tcp_stream_task<B>(proxy: &Proxy, request: MuxTcpStreamTask<B>)
where
    B: InboundMuxTcpRelay,
{
    let MuxTcpStreamTask {
        mut session,
        bridge,
        inbound_tag,
        protocol,
    } = request;
    let mux_session_id = bridge.mux_session_id();

    let upstream = match open_mux_tcp_upstream(proxy, &mut session, &inbound_tag).await {
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
    proxy: Proxy,
    session: Session,
    bridge: B,
    inbound_tag: String,
    protocol: &'static str,
) where
    B: InboundMuxTcpRelay,
{
    run_mux_tcp_stream_task(
        &proxy,
        MuxTcpStreamTask {
            session,
            bridge,
            inbound_tag,
            protocol,
        },
    )
    .await;
}

pub(crate) async fn open_mux_tcp_upstream(
    proxy: &Proxy,
    session: &mut Session,
    inbound_tag: &str,
) -> Result<TcpRouteResult, EngineError> {
    proxy.prepare_session(session, inbound_tag, None);
    TcpPipe::new(proxy).dispatch(TcpPipeInput { session }).await
}
