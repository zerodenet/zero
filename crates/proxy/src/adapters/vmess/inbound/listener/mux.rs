//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::sync::Mutex;

use tokio::task::JoinSet;
use tracing::warn;
use zero_engine::EngineError;

use crate::runtime::mux_session::{run_mux_session_loop, MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamTask};
use crate::runtime::Proxy;

use super::mux_udp::spawn_vmess_mux_udp_stream_task;

struct VmessMuxOpenedDispatcher<'a, R> {
    proxy: &'a Proxy,
    mux_server: &'a mut vmess::mux::VmessInboundMuxServer,
    reader: &'a mut R,
    inbound_tag: &'a str,
}

impl<R> MuxOpenedDispatcher for VmessMuxOpenedDispatcher<'_, R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    type Error = EngineError;

    async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
        let tasks = Mutex::new(Some(tasks));
        let writer = self.mux_server.writer();
        let proxy = self.proxy;
        let inbound_tag = self.inbound_tag;
        match self
            .mux_server
            .dispatch_next_opened_route_with_handlers(
                self.reader,
                |session_id, session, up_rx| {
                    let tasks = tasks
                        .lock()
                        .expect("lock vmess mux tcp tasks")
                        .take()
                        .expect("single vmess mux tcp dispatch");
                    let writer = writer.clone();
                    let inbound_tag = inbound_tag.to_owned();
                    async move {
                        spawn_vmess_mux_tcp_stream_task(
                            proxy,
                            tasks,
                            session_id,
                            session,
                            up_rx,
                            writer,
                            inbound_tag,
                        );
                        Ok::<(), EngineError>(())
                    }
                },
                |session_id, up_rx, responder| {
                    let tasks = tasks
                        .lock()
                        .expect("lock vmess mux udp tasks")
                        .take()
                        .expect("single vmess mux udp dispatch");
                    let inbound_tag = inbound_tag.to_owned();
                    async move {
                        spawn_vmess_mux_udp_stream_task(
                            proxy,
                            tasks,
                            session_id,
                            up_rx,
                            responder,
                            inbound_tag,
                        );
                        Ok::<(), EngineError>(())
                    }
                },
            )
            .await
        {
            Ok(keep_running) => Ok(keep_running),
            Err(error) => {
                warn!(error = %error, "vmess mux frame read failed");
                Ok(false)
            }
        }
    }
}

pub(super) async fn run_vmess_mux_session<R>(
    proxy: &Proxy,
    mut reader: R,
    mut mux_server: vmess::mux::VmessInboundMuxServer,
    inbound_tag: &str,
) -> Result<(), EngineError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut mux_tasks: JoinSet<()> = JoinSet::new();
    let mut dispatcher = VmessMuxOpenedDispatcher {
        proxy,
        mux_server: &mut mux_server,
        reader: &mut reader,
        inbound_tag,
    };

    run_mux_session_loop(
        MuxSessionLoop {
            inbound_tag,
            protocol: "vmess_mux",
            panic_message: "vmess mux task panicked",
            abort_on_end: false,
        },
        &mut mux_tasks,
        &mut dispatcher,
    )
    .await?;

    Ok(())
}

fn spawn_vmess_mux_tcp_stream_task(
    proxy: &Proxy,
    tasks: &mut JoinSet<()>,
    mux_session_id: u16,
    session: zero_core::Session,
    up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    writer: vmess::mux::VmessInboundMuxWriter,
    inbound_tag: String,
) {
    let close_writer = writer.clone();
    spawn_mux_tcp_stream_task(
        proxy,
        tasks,
        MuxTcpStreamTask {
            mux_session_id,
            session,
            uplink: up_rx,
            close_stream: move || async move {
                let _ = close_writer.end_inbound_stream(mux_session_id);
            },
            relay_stream: move |session_id, up_rx, upstream| async move {
                vmess::mux::relay_inbound_mux_stream(session_id, up_rx, writer, upstream).await;
            },
            inbound_tag,
            protocol: "vmess_mux",
        },
    );
}
