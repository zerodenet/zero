//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::task::JoinSet;
use tracing::warn;
use zero_engine::EngineError;

use crate::runtime::mux_session::{run_mux_session_loop, MuxOpenedDispatcher, MuxSessionLoop};
use crate::runtime::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamBridge, MuxTcpStreamTask};
use crate::runtime::Proxy;

use super::mux_udp::spawn_vmess_mux_udp_stream_task;

impl MuxTcpStreamBridge for vmess::mux::VmessInboundMuxTcpRelay {
    async fn close_stream(&self) {
        let _ = vmess::mux::VmessInboundMuxTcpRelay::close_stream(self);
    }

    async fn relay_stream(self, upstream: crate::transport::TcpRelayStream) {
        vmess::mux::VmessInboundMuxTcpRelay::relay_stream(self, upstream).await;
    }
}

struct VmessMuxOpenedDispatcherBridge<'a> {
    proxy: &'a Proxy,
    tasks: &'a mut JoinSet<()>,
    inbound_tag: &'a str,
}

impl vmess::mux::VmessInboundMuxOpenedRouteDispatcher for VmessMuxOpenedDispatcherBridge<'_> {
    type Error = EngineError;

    async fn dispatch_tcp_opened(
        &mut self,
        session: zero_core::Session,
        relay: vmess::mux::VmessInboundMuxTcpRelay,
    ) -> Result<(), Self::Error> {
        spawn_vmess_mux_tcp_stream_task(
            self.proxy,
            self.tasks,
            session,
            relay,
            self.inbound_tag.to_owned(),
        );
        Ok(())
    }

    async fn dispatch_udp_opened(
        &mut self,
        relay: vmess::mux::VmessInboundMuxUdpRelay,
    ) -> Result<(), Self::Error> {
        spawn_vmess_mux_udp_stream_task(self.proxy, self.tasks, relay, self.inbound_tag.to_owned());
        Ok(())
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
    struct OpenedDispatch<'a, R> {
        proxy: &'a Proxy,
        mux_server: &'a mut vmess::mux::VmessInboundMuxServer,
        reader: &'a mut R,
        inbound_tag: &'a str,
    }

    impl<R> MuxOpenedDispatcher for OpenedDispatch<'_, R>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        type Error = EngineError;

        async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error> {
            let mut bridge = VmessMuxOpenedDispatcherBridge {
                proxy: self.proxy,
                tasks,
                inbound_tag: self.inbound_tag,
            };
            match self
                .mux_server
                .dispatch_next_opened_route(self.reader, &mut bridge)
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

    let mut mux_tasks: JoinSet<()> = JoinSet::new();
    let mut dispatcher = OpenedDispatch {
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
    session: zero_core::Session,
    relay: vmess::mux::VmessInboundMuxTcpRelay,
    inbound_tag: String,
) {
    spawn_mux_tcp_stream_task(
        proxy,
        tasks,
        MuxTcpStreamTask {
            mux_session_id: relay.session_id(),
            session,
            bridge: relay,
            inbound_tag,
            protocol: "vmess_mux",
        },
    );
}
