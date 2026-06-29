//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::select;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_received,
    record_direct_udp_response_received, record_upstream_udp_response_received,
    wait_for_upstream_idle,
};
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

    pub(crate) async fn run_vmess_udp_relay(
        &self,
        mut client: TcpRelayStream,
        session: Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let mut udp_session = vmess::VmessInbound.udp_session_for(&session);
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vmess_udp",
            "vmess udp session started"
        );

        let mut client_buf = vec![0_u8; 64 * 1024];
        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "vmess_udp",
                        "vmess udp session idle timeout"
                    );
                    break;
                }
                read = udp_session.read_inbound_dispatch_tokio(&mut client, &mut client_buf) => {
                    match read {
                        Ok(None) => break,
                        Ok(Some(inbound_dispatch)) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput::from_inbound_dispatch(
                                    &inbound_dispatch,
                                    auth.as_ref(),
                                ))
                                .await
                            {
                                warn!(error = %error, "failed to process vmess udp packet");
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "vmess udp client read/decode error");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();
                    let response_accounting =
                        record_direct_udp_response_received(self, &dispatch, sender, n);
                    let written = udp_session.write_response_to_socket_addr_tokio(
                        &mut client,
                        sender,
                        &direct_buf[..n],
                    ).await?;
                    response_accounting.record_sent(written);
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            let response = record_upstream_udp_response_received(
                                self,
                                &mut dispatch,
                                self.udp_upstream_idle_timeout(),
                                pkt,
                            );
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &response.target,
                                response.port,
                                &response.payload,
                            ).await?;
                            response.accounting.record_sent(written);
                        }
                        Err(error) => {
                            warn!(error = %error, "vmess udp socks5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            let response_accounting =
                                record_chain_udp_response_received(self, session_id, payload.len());
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await?;
                            response_accounting.record_sent(written);
                        }
                        Ok(Err(error)) => warn!(error = %error, "vmess udp chain response error"),
                        Err(error) => warn!(error = %error, "vmess udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "vmess_udp",
            "vmess udp session ended"
        );

        Ok(())
    }
}
