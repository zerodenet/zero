use tokio::task::JoinSet;
use tracing::info;

use crate::runtime::mux_session::{run_mux_session_loop, MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamBridge, MuxTcpStreamTask};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

use super::mux_udp::spawn_vless_mux_udp_stream_task;

impl MuxTcpStreamBridge for vless::mux::VlessInboundMuxTcpRelay {
    async fn close_stream(&self) {
        let _ = vless::mux::VlessInboundMuxTcpRelay::close_stream(self);
    }

    async fn relay_stream(self, upstream: crate::transport::TcpRelayStream) {
        vless::mux::VlessInboundMuxTcpRelay::relay_stream(self, upstream).await;
    }
}

struct VlessMuxOpenedDispatcherBridge<'a> {
    proxy: &'a Proxy,
    tasks: &'a mut JoinSet<()>,
    inbound_tag: &'a str,
}

impl vless::mux::VlessInboundMuxOpenedRouteDispatcher for VlessMuxOpenedDispatcherBridge<'_> {
    type Error = EngineError;

    async fn dispatch_tcp_opened(
        &mut self,
        session: zero_core::Session,
        relay: vless::mux::VlessInboundMuxTcpRelay,
    ) -> Result<bool, Self::Error> {
        spawn_mux_tcp_stream_task(
            self.proxy,
            self.tasks,
            MuxTcpStreamTask {
                mux_session_id: relay.session_id(),
                session,
                bridge: relay,
                inbound_tag: self.inbound_tag.to_owned(),
                protocol: "vless_mux",
            },
        );
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
        let proxy = self.proxy.clone();
        let inbound_tag = self.inbound_tag.to_owned();
        info!(
            inbound_tag = %inbound_tag,
            mux_stream_id = session_id,
            port,
            network = "udp",
            "MUX stream accepted"
        );
        self.tasks.spawn(async move {
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
        Ok(true)
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
    struct OpenedDispatch<'a, S> {
        proxy: &'a Proxy,
        mux_server: &'a mut vless::mux::VlessInboundMuxServer,
        client: &'a mut MeteredStream<S>,
        inbound_tag: &'a str,
    }

    impl<S> MuxOpenedDispatcher for OpenedDispatch<'_, S>
    where
        S: ClientStream,
    {
        type Error = EngineError;

        async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
            let mut bridge = VlessMuxOpenedDispatcherBridge {
                proxy: self.proxy,
                tasks,
                inbound_tag: self.inbound_tag,
            };
            self.mux_server
                .dispatch_next_opened_route(self.client, &mut bridge)
                .await
        }
    }

    let mut relay_tasks = JoinSet::new();
    let mut dispatcher = OpenedDispatch {
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
