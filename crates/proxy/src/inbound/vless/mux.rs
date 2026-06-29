use std::collections::HashMap;
use tokio::select;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_udp_inbound_response_rx, record_udp_inbound_response_tx,
    udp_response_session_id, wait_for_upstream_idle,
};

use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, TcpRelayStream};
use zero_engine::EngineError;

use super::model::VlessMuxUdpStreamTask;

impl Proxy {
    pub(crate) async fn handle_vless_mux_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        uuid: [u8; 16],
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        use tokio::sync::mpsc;
        use vless::{MuxNetwork, MuxServerEvent, VlessInboundMuxSession};

        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(0, client.drain_traffic());

        let mut mux = VlessInboundMuxSession::with_encryption(&uuid);
        let mut up_senders: HashMap<u16, mpsc::UnboundedSender<Vec<u8>>> = HashMap::new();
        let mut relay_tasks = JoinSet::new();
        let (down_tx, mut down_rx) = mpsc::unbounded_channel::<(u16, Vec<u8>)>();

        info!(inbound_tag, "VLESS MUX session started");
        loop {
            tokio::select! {
                event_res = mux.next_event(&mut client) => {
                    let event = match event_res {
                        Ok(event) => event,
                        Err(_) => break,
                    };
                    match event {
                        MuxServerEvent::KeepAlive => {
                            // Keep-alive -?ignore
                            continue;
                        }
                        MuxServerEvent::NewStream { session_id: sid, target } => {
                            match target.network_kind() {
                                Ok(network) => {
                                    if mux.accept_stream(&mut client, sid).await.is_err() {
                                        break;
                                    }

                                    let (up_tx, up_rx) = mpsc::unbounded_channel();
                                    up_senders.insert(sid, up_tx);

                                    match network {
                                        MuxNetwork::Tcp => {
                                            // Route and establish TCP outbound
                                            let mut session = zero_core::Session::new(
                                                0, target.address, target.port,
                                                zero_core::Network::Tcp,
                                                zero_core::ProtocolType::Vless,
                                            );
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
                                                    let _ = mux.reject_stream(&mut client).await;
                                                    up_senders.remove(&sid);
                                                    continue;
                                                }
                                            };

                                            let down = down_tx.clone();
                                            relay_tasks.spawn(async move {
                                                Self::mux_stream_relay(sid, up_rx, down, upstream).await;
                                            });

                                            info!(inbound_tag, mux_stream_id = sid,
                                                port = target.port, network = "tcp",
                                                "MUX stream accepted");
                                        }
                                        MuxNetwork::Udp => {
                                            let down = down_tx.clone();
                                            let proxy_clone = self.clone();
                                            let inbound_tag_owned = inbound_tag.to_owned();
                                            let auth_clone = auth.clone();
                                            relay_tasks.spawn(async move {
                                                proxy_clone
                                                    .spawn_vless_mux_udp_stream_task(
                                                        VlessMuxUdpStreamTask {
                                                            mux_session_id: sid,
                                                            up_rx,
                                                            down_tx: down,
                                                            inbound_tag: &inbound_tag_owned,
                                                            auth: auth_clone.as_ref(),
                                                        },
                                                    )
                                                    .await;
                                            });

                                            info!(inbound_tag, mux_stream_id = sid,
                                                port = target.port, network = "udp",
                                                "MUX stream accepted");
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "MUX new stream parse failed");
                                    let _ = mux.reject_stream(&mut client).await;
                                }
                            }
                        }
                        MuxServerEvent::Data { session_id, payload } => {
                            // Data for an existing stream
                            if let Some(tx) = up_senders.get(&session_id) {
                                let _ = tx.send(payload);
                            } else {
                                // Data for unknown stream -?ignore or send END
                                let _ =
                                    mux.end_stream(&mut client, session_id).await;
                            }
                        }
                        MuxServerEvent::End { session_id } => {
                            // Client terminated this stream
                            up_senders.remove(&session_id);
                            info!(mux_stream_id = session_id,
                                "MUX stream ended by client");
                        }
                        MuxServerEvent::Unknown { .. } => {
                            // Unknown status -?ignore
                        }
                    }
                }

                down = down_rx.recv() => {
                    if let Some((sid, payload)) = down {
                        if up_senders.contains_key(&sid) {
                            if payload.is_empty() {
                                // Upstream closed -?send END frame and clean up
                                let _ = mux.end_stream(&mut client, sid).await;
                                up_senders.remove(&sid);
                            } else {
                                let _ = mux.send_data(&mut client, sid, &payload).await;
                            }
                        }
                    }
                }
            }
        }

        relay_tasks.abort_all();
        info!(inbound_tag, "VLESS MUX session ended");
        Ok(())
    }

    pub(crate) async fn mux_stream_relay(
        stream_id: u16,
        mut up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        down_tx: tokio::sync::mpsc::UnboundedSender<(u16, Vec<u8>)>,
        upstream: TcpRelayStream,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut upstream_r, mut upstream_w) = tokio::io::split(upstream);

        let upload = tokio::spawn(async move {
            while let Some(data) = up_rx.recv().await {
                if upstream_w.write_all(&data).await.is_err() {
                    break;
                }
            }
            let _ = upstream_w.shutdown().await;
        });

        let sid = stream_id;
        let download = tokio::spawn(async move {
            let mut buf = [0u8; 16384];
            loop {
                match upstream_r.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if down_tx.send((sid, buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            // Send empty payload as close notification
            let _ = down_tx.send((sid, vec![]));
        });

        let _ = tokio::join!(upload, download);
    }

    pub(crate) async fn spawn_vless_mux_udp_stream_task(&self, request: VlessMuxUdpStreamTask<'_>) {
        let VlessMuxUdpStreamTask {
            mux_session_id,
            up_rx,
            down_tx,
            inbound_tag,
            auth,
        } = request;
        let mut up_rx = up_rx;
        let mut dispatch = match UdpDispatch::new(inbound_tag).await {
            Ok(dispatch) => dispatch,
            Err(error) => {
                warn!(%error, mux_session_id, "vless mux udp dispatch init failed");
                let _ = down_tx.send((mux_session_id, vec![]));
                return;
            }
        };
        let timeout = self.udp_upstream_idle_timeout();
        let mut last_activity = TokioInstant::now();
        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];
        let udp_session = vless::VlessInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless_mux_udp",
            mux_session_id,
            "vless mux udp sub-stream started"
        );

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();
            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "vless_mux_udp",
                        mux_session_id,
                        "vless mux udp sub-stream idle timeout"
                    );
                    break;
                }
                payload = up_rx.recv() => {
                    let Some(payload) = payload else { break; };
                    if payload.is_empty() {
                        break;
                    }
                    last_activity = TokioInstant::now();
                    let request = match udp_session.decode_request(&payload) {
                        Ok(request) => request,
                        Err(error) => {
                            warn!(%error, mux_session_id, "vless mux udp packet parse failed");
                            continue;
                        }
                    };
                    let request = request.into_dispatch_parts();
                    if let Err(error) = UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target: request.target,
                            port: request.port,
                            payload: &request.payload,
                            protocol: zero_core::ProtocolType::Vless,
                            auth,
                            client_session_id: request.client_session_id,
                        })
                        .await
                    {
                        warn!(%error, mux_session_id, "vless mux udp packet dispatch failed");
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    match recv {
                        Ok((n, sender)) => {
                            last_activity = TokioInstant::now();
                            let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                            let session_id = dispatch.direct_response_session_id(sender);
                            record_udp_inbound_response_rx(self, session_id, n);
                            match udp_session.send_mux_response_to_ip(
                                &down_tx,
                                mux_session_id,
                                ip,
                                sender.port(),
                                &direct_buf[..n],
                            ) {
                                Ok(frame_len) => {
                                    record_udp_inbound_response_tx(self, session_id, frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp direct response encode failed");
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(%error, mux_session_id, "vless mux udp direct recv failed");
                            break;
                        }
                    }
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(timeout);
                            let (target, port, payload) = pkt.into_parts();
                            let session_id = udp_response_session_id(&dispatch, &target, port);
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            match udp_session.send_mux_response(
                                &down_tx,
                                mux_session_id,
                                &target,
                                port,
                                &payload,
                            ) {
                                Ok(frame_len) => {
                                    record_udp_inbound_response_tx(self, session_id, frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp upstream response encode failed");
                                    break;
                                }
                            }
                        }
                        Err(error) => warn!(%error, mux_session_id, "vless mux udp socks5 upstream recv failed"),
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            match udp_session.send_mux_response(
                                &down_tx,
                                mux_session_id,
                                &target,
                                port,
                                &payload,
                            ) {
                                Ok(frame_len) => {
                                    record_udp_inbound_response_tx(self, session_id, frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp chain response encode failed");
                                    break;
                                }
                            }
                        }
                        Ok(Err(error)) => warn!(%error, mux_session_id, "vless mux udp chain response failed"),
                        Err(error) => warn!(%error, mux_session_id, "vless mux udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }
        let _ = down_tx.send((mux_session_id, vec![]));
    }
}
