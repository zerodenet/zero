use tokio::task::JoinSet;
use tracing::info;

use crate::runtime::mux_session::{run_mux_session_loop, MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamTask};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

struct VlessMuxOpenedDispatcher<'a, S> {
    proxy: &'a Proxy,
    mux_server: &'a mut vless::mux::VlessInboundMuxServer,
    client: &'a mut MeteredStream<S>,
    inbound_tag: &'a str,
}

struct VlessMuxOpenedRouteBridge<'a> {
    proxy: &'a Proxy,
    writer: vless::mux::VlessInboundMuxWriter,
    inbound_tag: &'a str,
    tasks: &'a mut JoinSet<()>,
}

impl<S> MuxOpenedDispatcher for VlessMuxOpenedDispatcher<'_, S>
where
    S: ClientStream,
{
    type Error = EngineError;

    async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
        let writer = self.mux_server.writer();
        let mut bridge = VlessMuxOpenedRouteBridge {
            proxy: self.proxy,
            writer,
            inbound_tag: self.inbound_tag,
            tasks,
        };
        self.mux_server
            .dispatch_next_opened_route(self.client, &mut bridge)
            .await
    }
}

impl VlessMuxOpenedRouteBridge<'_> {
    fn bridge_tcp_opened(
        &mut self,
        session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        let writer = self.writer.clone();
        let close_writer = self.writer.clone();
        let inbound_tag = self.inbound_tag.to_owned();
        spawn_mux_tcp_stream_task(
            self.proxy,
            self.tasks,
            MuxTcpStreamTask {
                mux_session_id: session_id,
                session,
                uplink: up_rx,
                close_stream: move || async move {
                    let _ = close_writer.end_inbound_stream(session_id);
                },
                relay_stream: move |session_id, up_rx, upstream| async move {
                    vless::mux::relay_inbound_mux_stream(session_id, up_rx, writer, upstream).await;
                },
                inbound_tag,
                protocol: "vless_mux",
            },
        );
    }

    fn bridge_udp_opened(
        &mut self,
        session_id: u16,
        port: u16,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        responder: vless::udp::VlessInboundMuxUdpResponder,
        auth: Option<zero_core::SessionAuth>,
    ) {
        let proxy_clone = self.proxy.clone();
        let inbound_tag_owned = self.inbound_tag.to_owned();
        self.tasks.spawn(async move {
            proxy_clone
                .spawn_vless_mux_udp_stream_task(
                    session_id,
                    up_rx,
                    responder,
                    &inbound_tag_owned,
                    auth,
                )
                .await;
        });

        info!(
            inbound_tag = self.inbound_tag,
            mux_stream_id = session_id,
            port,
            network = "udp",
            "MUX stream accepted"
        );
    }
}

impl vless::mux::VlessInboundMuxOpenedRouteDispatcher for VlessMuxOpenedRouteBridge<'_> {
    type Error = EngineError;

    async fn dispatch_tcp_opened(
        &mut self,
        session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Result<bool, Self::Error> {
        self.bridge_tcp_opened(session_id, session, up_rx);
        Ok(true)
    }

    async fn dispatch_udp_opened(
        &mut self,
        session_id: u16,
        port: u16,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        responder: vless::udp::VlessInboundMuxUdpResponder,
        auth: Option<zero_core::SessionAuth>,
    ) -> Result<bool, Self::Error> {
        self.bridge_udp_opened(session_id, port, up_rx, responder, auth);
        Ok(true)
    }
}

impl Proxy {
    pub(crate) async fn handle_vless_mux_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        mut mux_server: vless::mux::VlessInboundMuxServer,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut relay_tasks = JoinSet::new();
        let mut dispatcher = VlessMuxOpenedDispatcher {
            proxy: self,
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
}
