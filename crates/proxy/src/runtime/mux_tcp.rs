use std::future::Future;

use tokio::task::JoinSet;
use tracing::warn;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::{TcpRelayStream, TcpRouteResult};

pub(crate) struct MuxTcpStreamTask<U, C, R> {
    pub(crate) mux_session_id: u16,
    pub(crate) session: Session,
    pub(crate) uplink: U,
    pub(crate) close_stream: C,
    pub(crate) relay_stream: R,
    pub(crate) inbound_tag: String,
    pub(crate) protocol: &'static str,
}

pub(crate) fn spawn_mux_tcp_stream_task<U, C, CFut, R, RFut>(
    proxy: &Proxy,
    tasks: &mut JoinSet<()>,
    request: MuxTcpStreamTask<U, C, R>,
) where
    U: Send + 'static,
    C: FnOnce() -> CFut + Send + 'static,
    CFut: Future<Output = ()> + Send + 'static,
    R: FnOnce(u16, U, TcpRelayStream) -> RFut + Send + 'static,
    RFut: Future<Output = ()> + Send + 'static,
{
    let proxy = proxy.clone();
    tasks.spawn(async move {
        let MuxTcpStreamTask {
            mux_session_id,
            mut session,
            uplink,
            close_stream,
            relay_stream,
            inbound_tag,
            protocol,
        } = request;

        let upstream = match open_mux_tcp_upstream(&proxy, &mut session, &inbound_tag).await {
            Ok(result) => result.upstream,
            Err(error) => {
                warn!(%error, mux_session_id, protocol, "mux tcp dispatch failed");
                close_stream().await;
                return;
            }
        };

        relay_stream(mux_session_id, uplink, upstream).await;
    });
}

pub(crate) async fn open_mux_tcp_upstream(
    proxy: &Proxy,
    session: &mut Session,
    inbound_tag: &str,
) -> Result<TcpRouteResult, EngineError> {
    proxy.prepare_session(session, inbound_tag, None);
    TcpPipe::new(proxy).dispatch(TcpPipeInput { session }).await
}
