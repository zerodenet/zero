use std::sync::Mutex;

use tokio::task::JoinSet;
use tracing::info;

use crate::runtime::mux_session::{run_mux_session_loop, MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamTask};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

use super::mux_udp::spawn_vless_mux_udp_stream_task;

struct VlessMuxOpenedDispatcher<'a, S> {
    proxy: &'a Proxy,
    mux_server: &'a mut vless::mux::VlessInboundMuxServer,
    client: &'a mut MeteredStream<S>,
    inbound_tag: &'a str,
}

impl<S> MuxOpenedDispatcher for VlessMuxOpenedDispatcher<'_, S>
where
    S: ClientStream,
{
    type Error = EngineError;

    async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
        let tasks = Mutex::new(Some(tasks));
        let writer = self.mux_server.writer();
        let proxy = self.proxy;
        let inbound_tag = self.inbound_tag;
        self.mux_server
            .dispatch_next_opened_route_with_handlers(
                self.client,
                |session_id, session, up_rx| {
                    let tasks = tasks
                        .lock()
                        .expect("lock vless mux tcp tasks")
                        .take()
                        .expect("single vless mux tcp dispatch");
                    let writer = writer.clone();
                    let close_writer = writer.clone();
                    let inbound_tag = inbound_tag.to_owned();
                    async move {
                        spawn_mux_tcp_stream_task(
                            proxy,
                            tasks,
                            MuxTcpStreamTask {
                                mux_session_id: session_id,
                                session,
                                uplink: up_rx,
                                close_stream: move || async move {
                                    let _ = close_writer.end_inbound_stream(session_id);
                                },
                                relay_stream: move |session_id, up_rx, upstream| async move {
                                    vless::mux::relay_inbound_mux_stream(
                                        session_id, up_rx, writer, upstream,
                                    )
                                    .await;
                                },
                                inbound_tag,
                                protocol: "vless_mux",
                            },
                        );
                        Ok::<bool, EngineError>(true)
                    }
                },
                |session_id, port, up_rx, responder, auth| {
                    let tasks = tasks
                        .lock()
                        .expect("lock vless mux udp tasks")
                        .take()
                        .expect("single vless mux udp dispatch");
                    let proxy = proxy.clone();
                    let inbound_tag = inbound_tag.to_owned();
                    async move {
                        info!(
                            inbound_tag = %inbound_tag,
                            mux_stream_id = session_id,
                            port,
                            network = "udp",
                            "MUX stream accepted"
                        );
                        tasks.spawn(async move {
                            spawn_vless_mux_udp_stream_task(
                                &proxy,
                                session_id,
                                up_rx,
                                responder,
                                &inbound_tag,
                                auth,
                            )
                            .await;
                        });
                        Ok::<bool, EngineError>(true)
                    }
                },
            )
            .await
    }
}

pub(super) async fn handle_vless_mux_session<S>(
    proxy: &Proxy,
    mut client: MeteredStream<S>,
    inbound_tag: &str,
    mut mux_server: vless::mux::VlessInboundMuxServer,
) -> Result<(), EngineError>
where
    S: ClientStream,
{
    let mut relay_tasks = JoinSet::new();
    let mut dispatcher = VlessMuxOpenedDispatcher {
        proxy,
        mux_server: &mut mux_server,
        client: &mut client,
        inbound_tag,
    };

    run_mux_session_loop(
        MuxSessionLoop {
            inbound_tag,
            protocol: "vless_mux",
            panic_message: "vless mux task panicked",
            abort_on_end: true,
        },
        &mut relay_tasks,
        &mut dispatcher,
    )
    .await
}
