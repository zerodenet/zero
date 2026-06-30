//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_engine::EngineError;

use crate::inbound::mux_tcp::{spawn_mux_tcp_stream_task, MuxTcpStreamTask};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

struct VmessMuxOpenedHandler<'a> {
    proxy: &'a Proxy,
    tasks: &'a mut JoinSet<()>,
    inbound_tag: &'a str,
}

impl vmess::mux::VmessInboundMuxOpenedHandler for VmessMuxOpenedHandler<'_> {
    type Error = EngineError;

    async fn handle_tcp_opened(
        &mut self,
        session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
    ) -> Result<(), Self::Error> {
        self.proxy.spawn_vmess_mux_tcp_stream_task(
            self.tasks,
            session_id,
            session,
            up_rx,
            writer,
            self.inbound_tag.to_owned(),
        );
        Ok(())
    }

    async fn handle_udp_opened(
        &mut self,
        session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
    ) -> Result<(), Self::Error> {
        self.proxy.spawn_vmess_mux_udp_stream_task(
            self.tasks,
            session_id,
            session,
            up_rx,
            writer,
            self.inbound_tag.to_owned(),
        );
        Ok(())
    }
}

impl Proxy {
    pub(crate) async fn run_vmess_mux_session(
        &self,
        client: TcpRelayStream,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let (mut reader, writer) = tokio::io::split(client);
        let mut mux_server = vmess::mux::VmessInboundMuxServer::from_tokio_writer(writer);
        let mut mux_tasks: JoinSet<()> = JoinSet::new();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vmess_mux",
            "vmess mux session started"
        );

        loop {
            let mut handler = VmessMuxOpenedHandler {
                proxy: self,
                tasks: &mut mux_tasks,
                inbound_tag,
            };
            if let Err(error) = mux_server
                .dispatch_next_opened_stream(&mut reader, &mut handler)
                .await
            {
                warn!(error = %error, "vmess mux frame read failed");
                break;
            }

            while let Some(joined) = mux_tasks.try_join_next() {
                if let Err(error) = joined {
                    warn!(error = %error, "vmess mux task panicked");
                }
            }
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "vmess_mux",
            "vmess mux session ended"
        );
        Ok(())
    }

    fn spawn_vmess_mux_tcp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        session: zero_core::Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
        inbound_tag: String,
    ) {
        let close_writer = writer.clone();
        spawn_mux_tcp_stream_task(
            self,
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
}
