use std::future::Future;

use tokio::task::JoinSet;
use tracing::warn;
use zero_core::Session;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) fn spawn_mux_tcp_stream_task<U, C, CFut, R, RFut>(
    proxy: &Proxy,
    tasks: &mut JoinSet<()>,
    mux_session_id: u16,
    mut session: Session,
    uplink: U,
    close_stream: C,
    relay_stream: R,
    inbound_tag: String,
    protocol: &'static str,
) where
    U: Send + 'static,
    C: FnOnce() -> CFut + Send + 'static,
    CFut: Future<Output = ()> + Send + 'static,
    R: FnOnce(u16, U, TcpRelayStream) -> RFut + Send + 'static,
    RFut: Future<Output = ()> + Send + 'static,
{
    let proxy = proxy.clone();
    tasks.spawn(async move {
        proxy.prepare_session(&mut session, &inbound_tag, None);

        let upstream = match TcpPipe::new(&proxy)
            .dispatch(TcpPipeInput {
                session: &mut session,
            })
            .await
        {
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
