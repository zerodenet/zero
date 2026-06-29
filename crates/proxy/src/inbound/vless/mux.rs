use tokio::task::JoinSet;
use tracing::info;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};

use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn handle_vless_mux_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        mux_context: vless::mux::VlessInboundMuxContext,
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(0, client.drain_traffic());

        let mut mux_server = vless::mux::VlessInboundMuxServer::from_context(mux_context);
        let mut relay_tasks = JoinSet::new();

        info!(inbound_tag, "VLESS MUX session started");
        loop {
            tokio::select! {
                event = mux_server.next_opened_stream(&mut client) => {
                    let event = match event {
                        Ok(event) => event,
                        Err(_) => break,
                    };
                    match event {
                        Some(vless::mux::VlessInboundMuxEvent::Opened(opened)) => {
                            match opened.into_kind() {
                                vless::mux::VlessInboundMuxOpenedKind::Tcp {
                                    session_id: sid,
                                    mut session,
                                    up_rx,
                                } => {
                                    // Route and establish TCP outbound
                                    if let Some(ref a) = auth {
                                        session.apply_auth(a.clone());
                                    }
                                    self.prepare_session(&mut session, inbound_tag, None);
                                    let upstream = match TcpPipe::new(self)
                                        .dispatch(TcpPipeInput {
                                            session: &mut session,
                                        })
                                        .await
                                    {
                                        Ok(result) => result.upstream,
                                        Err(_) => {
                                            let _ = mux_server.reject_opened_stream(&mut client, sid).await;
                                            continue;
                                        }
                                    };

                                    let writer = mux_server.writer();
                                    relay_tasks.spawn(async move {
                                        vless::mux::relay_inbound_mux_stream(
                                            sid, up_rx, writer, upstream,
                                        )
                                        .await;
                                    });

                                    info!(inbound_tag, mux_stream_id = sid,
                                        port = session.port, network = "tcp",
                                        "MUX stream accepted");
                                }
                                vless::mux::VlessInboundMuxOpenedKind::Udp {
                                    session_id: sid,
                                    session,
                                    up_rx,
                                } => {
                                    let proxy_clone = self.clone();
                                    let inbound_tag_owned = inbound_tag.to_owned();
                                    let auth_clone = auth.clone();
                                    let writer = mux_server.writer();
                                    relay_tasks.spawn(async move {
                                        proxy_clone
                                            .spawn_vless_mux_udp_stream_task(
                                                sid,
                                                up_rx,
                                                writer,
                                                &inbound_tag_owned,
                                                auth_clone.as_ref(),
                                            )
                                            .await;
                                    });

                                    info!(inbound_tag, mux_stream_id = sid,
                                        port = session.port, network = "udp",
                                        "MUX stream accepted");
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
        }

        relay_tasks.abort_all();
        info!(inbound_tag, "VLESS MUX session ended");
        Ok(())
    }
}
