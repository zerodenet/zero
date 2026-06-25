use std::collections::HashMap;
use tokio::select;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_traits::AsyncSocket;

use crate::protocol_runtime::socks5_udp::recv_upstream_packet;
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{log_completed_udp_flow, wait_for_upstream_idle};

use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, TcpRelayStream};
use zero_engine::EngineError;

use super::model::VlessMuxUdpStreamTask;
use super::{decode_vless_udp_packet, encode_vless_mux_udp_response};

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
        use vless::{
            encode_new_stream_response, parse_new_stream, MuxServer, MUX_STATUS_FAIL,
            MUX_STATUS_OK, NETWORK_TCP, NETWORK_UDP, STATUS_END, STATUS_KEEP, STATUS_KEEP_ALIVE,
            STATUS_NEW,
        };

        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(0, client.drain_traffic());

        let mut mux = MuxServer::with_encryption(&uuid);
        let mut next_id: u16 = 1;
        let mut up_senders: HashMap<u16, mpsc::UnboundedSender<Vec<u8>>> = HashMap::new();
        let mut relay_tasks = JoinSet::new();
        let (down_tx, mut down_rx) = mpsc::unbounded_channel::<(u16, Vec<u8>)>();

        info!(inbound_tag, "VLESS MUX session started");
        loop {
            tokio::select! {
                frame_res = mux.recv(&mut client) => {
                    let frame = match frame_res {
                        Ok(f) => f,
                        Err(_) => break,
                    };
                    match frame.status {
                        STATUS_KEEP_ALIVE => {
                            // Keep-alive -?ignore
                            continue;
                        }
                        STATUS_NEW => {
                            // New stream request (session_id == 0)
                            match parse_new_stream(&frame.payload) {
                                Ok(target) => {
                                    let sid = next_id;
                                    next_id = next_id.wrapping_add(1);
                                    if next_id == 0 { next_id = 1; }

                                    // Write response directly (not encrypted, not wrapped in keep)
                                    let resp = encode_new_stream_response(sid, MUX_STATUS_OK);
                                    if client.write_all(&resp).await.is_err() {
                                        break;
                                    }

                                    let (up_tx, up_rx) = mpsc::unbounded_channel();
                                    up_senders.insert(sid, up_tx);

                                    match target.network {
                                        NETWORK_TCP => {
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
                                                    let fail_resp = encode_new_stream_response(
                                                        0, MUX_STATUS_FAIL,
                                                    );
                                                    let _ = client.write_all(&fail_resp).await;
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
                                        NETWORK_UDP => {
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
                                        _ => {
                                            warn!("MUX new stream unknown network {}", target.network);
                                            let fail_resp = encode_new_stream_response(
                                                0, MUX_STATUS_FAIL,
                                            );
                                            let _ = client.write_all(&fail_resp).await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "MUX new stream parse failed");
                                    let resp = encode_new_stream_response(0, MUX_STATUS_FAIL);
                                    let _ = client.write_all(&resp).await;
                                }
                            }
                        }
                        STATUS_KEEP => {
                            // Data for an existing stream
                            if let Some(tx) = up_senders.get(&frame.session_id) {
                                let _ = tx.send(frame.payload);
                            } else {
                                // Data for unknown stream -?ignore or send END
                                let _ =
                                    mux.write_end(&mut client, frame.session_id).await;
                            }
                        }
                        STATUS_END => {
                            // Client terminated this stream
                            up_senders.remove(&frame.session_id);
                            info!(mux_stream_id = frame.session_id,
                                "MUX stream ended by client");
                        }
                        _ => {
                            // Unknown status -?ignore
                        }
                    }
                }

                down = down_rx.recv() => {
                    if let Some((sid, payload)) = down {
                        if up_senders.contains_key(&sid) {
                            if payload.is_empty() {
                                // Upstream closed -?send END frame and clean up
                                let _ = mux.write_end(&mut client, sid).await;
                                up_senders.remove(&sid);
                            } else {
                                let _ = mux.write_data(&mut client, sid, &payload).await;
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

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless_mux_udp",
            mux_session_id,
            "vless mux udp sub-stream started"
        );

        loop {
            let (direct_sock, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();
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
                    let packet = match decode_vless_udp_packet(&payload) {
                        Ok(packet) => packet,
                        Err(error) => {
                            warn!(%error, mux_session_id, "vless mux udp packet parse failed");
                            continue;
                        }
                    };
                    if let Err(error) = UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target: packet.target,
                            port: packet.port,
                            payload: &packet.payload,
                            protocol: zero_core::ProtocolType::Vless,
                            auth,
                            client_session_id: None,
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
                            let target = match zero_platform_tokio::socket_addr_to_ip(sender) {
                                zero_traits::IpAddress::V4(bytes) => zero_core::Address::Ipv4(bytes),
                                zero_traits::IpAddress::V6(bytes) => zero_core::Address::Ipv6(bytes),
                            };
                            if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                self.record_session_outbound_rx(sid, n as u64);
                            }
                            let frame = encode_vless_mux_udp_response(
                                mux_session_id,
                                &target,
                                sender.port(),
                                &direct_buf[..n],
                            );
                            match frame {
                                Ok(frame) => {
                                    let frame_len = frame.len() as u64;
                                    if down_tx.send((mux_session_id, frame)).is_err() {
                                        break;
                                    }
                                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                        self.record_session_inbound_tx(sid, frame_len);
                                    }
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
                upstream = recv_upstream_packet(socks5_up, &mut upstream_buf) => {
                    match upstream {
                        Ok(read) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_socks5_idle(timeout);
                            if let Ok(pkt) = socks5::decode_udp_associate_response(&upstream_buf[..read]) {
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                    self.record_session_outbound_rx(sid, pkt.payload.len() as u64);
                                }
                                match encode_vless_mux_udp_response(
                                    mux_session_id,
                                    &pkt.target,
                                    pkt.port,
                                    &pkt.payload,
                                ) {
                                    Ok(frame) => {
                                        let frame_len = frame.len() as u64;
                                        if down_tx.send((mux_session_id, frame)).is_err() {
                                            break;
                                        }
                                        if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                            self.record_session_inbound_tx(sid, frame_len);
                                        }
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vless mux udp upstream response encode failed");
                                        break;
                                    }
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
                            if let Some(sid) = session_id {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            match encode_vless_mux_udp_response(
                                mux_session_id,
                                &target,
                                port,
                                &payload,
                            ) {
                                Ok(frame) => {
                                    let frame_len = frame.len() as u64;
                                    if down_tx.send((mux_session_id, frame)).is_err() {
                                        break;
                                    }
                                    if let Some(sid) = session_id {
                                        self.record_session_inbound_tx(sid, frame_len);
                                    }
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
