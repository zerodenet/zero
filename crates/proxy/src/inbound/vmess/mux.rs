//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::select;
use tokio::task::JoinSet;
use tracing::{info, warn};
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

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
            select! {
                opened = mux_server.read_opened_stream(&mut reader) => {
                    let opened = match opened {
                        Ok(opened) => opened,
                        Err(error) => {
                            warn!(error = %error, "vmess mux frame read failed");
                            break;
                        }
                    };

                    if let Some(opened) = opened {
                            match opened.into_kind() {
                                vmess::mux::VmessInboundMuxOpenedKind::Tcp {
                                    session_id,
                                    session,
                                    up_rx,
                                } => {
                                    self.spawn_vmess_mux_tcp_stream_task(
                                        &mut mux_tasks,
                                        session_id,
                                        session,
                                        up_rx,
                                        mux_server.writer(),
                                        inbound_tag.to_owned(),
                                    )
                                }
                                vmess::mux::VmessInboundMuxOpenedKind::Udp {
                                    session_id,
                                    session,
                                    up_rx,
                                } => {
                                    self.spawn_vmess_mux_udp_stream_task(
                                        &mut mux_tasks,
                                        session_id,
                                        session,
                                        up_rx,
                                        mux_server.writer(),
                                        inbound_tag.to_owned(),
                                    )
                                }
                            }
                    }
                }
                Some(joined) = mux_tasks.join_next(), if !mux_tasks.is_empty() => {
                    if let Err(error) = joined {
                        warn!(error = %error, "vmess mux task panicked");
                    }
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

    pub(crate) fn spawn_vmess_mux_tcp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        session: Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
        inbound_tag: String,
    ) {
        let proxy = self.clone();
        tasks.spawn(async move {
            let mut session = session;
            proxy.prepare_session(&mut session, &inbound_tag, None);

            let upstream = match TcpPipe::new(&proxy)
                .dispatch(TcpPipeInput {
                    session: &mut session,
                })
                .await
            {
                Ok(result) => result.upstream,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux dispatch failed");
                    let _ = writer.end_inbound_stream(mux_session_id);
                    return;
                }
            };

            vmess::mux::relay_inbound_mux_stream(mux_session_id, up_rx, writer, upstream).await;
        });
    }
}
