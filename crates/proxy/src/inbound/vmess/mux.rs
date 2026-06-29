//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::{Network, ProtocolType, Session};
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_udp_inbound_response_rx, record_udp_inbound_response_tx,
    udp_response_session_id, wait_for_upstream_idle,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

use super::model::{VmessMuxTcpStreamTask, VmessMuxUdpStreamTask};

impl Proxy {
    pub(crate) async fn run_vmess_mux_session(
        &self,
        client: TcpRelayStream,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let (mut reader, mut writer) = tokio::io::split(client);
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let mux_writer = vmess::mux::VmessInboundMuxWriter::new(write_tx.clone());
        let mut mux_tasks: JoinSet<()> = JoinSet::new();
        let mux_session = vmess::mux::VmessInboundMuxSession::new();
        let mut streams: std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>> =
            std::collections::HashMap::new();

        mux_tasks.spawn(async move {
            while let Some(frame) = write_rx.recv().await {
                if tokio::io::AsyncWriteExt::write_all(&mut writer, &frame)
                    .await
                    .is_err()
                {
                    break;
                }
                if tokio::io::AsyncWriteExt::flush(&mut writer).await.is_err() {
                    break;
                }
            }
            let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
        });

        info!(
            inbound_tag = inbound_tag,
            protocol = "vmess_mux",
            "vmess mux session started"
        );

        loop {
            select! {
                action = mux_session.read_inbound_action(&mut reader) => {
                    let action = match action {
                        Ok(action) => action,
                        Err(error) => {
                            warn!(error = %error, "vmess mux frame read failed");
                            break;
                        }
                    };

                    match action {
                        vmess::mux::VmessInboundMuxAction::KeepAlive => continue,
                        vmess::mux::VmessInboundMuxAction::OpenStream {
                            session_id,
                            session,
                            initial_payload,
                        } => {
                            let session = *session;
                            let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                            streams.insert(session_id, up_tx.clone());
                            if !initial_payload.is_empty() {
                                let _ = up_tx.send(initial_payload);
                            }
                            match session.network {
                                Network::Tcp => {
                                    self.spawn_vmess_mux_tcp_stream_task(VmessMuxTcpStreamTask {
                                        tasks: &mut mux_tasks,
                                        mux_session_id: session_id,
                                        target: session.target,
                                        port: session.port,
                                        up_rx,
                                        writer: mux_writer.clone(),
                                        inbound_tag: inbound_tag.to_owned(),
                                    })
                                }
                                Network::Udp => {
                                    self.spawn_vmess_mux_udp_stream_task(VmessMuxUdpStreamTask {
                                        tasks: &mut mux_tasks,
                                        mux_session_id: session_id,
                                        default_target: session.target,
                                        default_port: session.port,
                                        up_rx,
                                        writer: mux_writer.clone(),
                                        inbound_tag: inbound_tag.to_owned(),
                                    })
                                }
                            }
                        }
                        vmess::mux::VmessInboundMuxAction::Data {
                            session_id,
                            payload,
                        } => {
                            if let Some(tx) = streams.get(&session_id) {
                                if !payload.is_empty() {
                                    let _ = tx.send(payload);
                                }
                            }
                        }
                        vmess::mux::VmessInboundMuxAction::End { session_id } => {
                            if let Some(tx) = streams.remove(&session_id) {
                                let _ = tx.send(Vec::new());
                            }
                        }
                        vmess::mux::VmessInboundMuxAction::Unknown { .. } => {}
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

    pub(crate) fn spawn_vmess_mux_tcp_stream_task(&self, request: VmessMuxTcpStreamTask<'_>) {
        let VmessMuxTcpStreamTask {
            tasks,
            mux_session_id,
            target,
            port,
            up_rx,
            writer,
            inbound_tag,
        } = request;
        let mut up_rx = up_rx;
        let proxy = self.clone();
        tasks.spawn(async move {
            let mux_session = vmess::mux::VmessInboundMuxSession::new();
            let mut session =
                Session::new(0, target, port, Network::Tcp, ProtocolType::Vmess);
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
                    let _ = mux_session.end_inbound_stream(&writer, mux_session_id);
                    return;
                }
            };

            let mut upstream = upstream;
            let mut buf = vec![0_u8; 16 * 1024];
            loop {
                select! {
                    payload = up_rx.recv() => {
                        let Some(payload) = payload else { break; };
                        if payload.is_empty() {
                            break;
                        }
                        if tokio::io::AsyncWriteExt::write_all(&mut upstream, &payload).await.is_err() {
                            break;
                        }
                        if tokio::io::AsyncWriteExt::flush(&mut upstream).await.is_err() {
                            break;
                        }
                    }
                    read = tokio::io::AsyncReadExt::read(&mut upstream, &mut buf) => {
                        match read {
                            Ok(0) => break,
                            Ok(n) => {
                                match mux_session.write_inbound_stream_payload(&writer, mux_session_id, &buf[..n]) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux response encode failed");
                                        break;
                                    }
                                }
                            }
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux upstream read failed");
                                break;
                            }
                        }
                    }
                }
            }
            let _ = mux_session.write_inbound_stream_payload(&writer, mux_session_id, &[]);
        });
    }

    pub(crate) fn spawn_vmess_mux_udp_stream_task(&self, request: VmessMuxUdpStreamTask<'_>) {
        let VmessMuxUdpStreamTask {
            tasks,
            mux_session_id,
            default_target,
            default_port,
            up_rx,
            writer,
            inbound_tag,
        } = request;
        let mut up_rx = up_rx;
        let proxy = self.clone();
        tasks.spawn(async move {
            let mux_session = vmess::mux::VmessInboundMuxSession::new();
            let mut udp_session =
                vmess::VmessInbound.udp_session(default_target, default_port);
            let mut dispatch = match UdpDispatch::new(&inbound_tag).await {
                Ok(dispatch) => dispatch,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux udp dispatch init failed");
                    let _ = mux_session.end_inbound_stream(&writer, mux_session_id);
                    return;
                }
            };
            let timeout = proxy.udp_upstream_idle_timeout();
            let mut last_activity = TokioInstant::now();
            let mut direct_buf = vec![0_u8; 64 * 1024];
            let mut upstream_buf = vec![0_u8; 64 * 1024];

            loop {
                let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();
                select! {
                    _ = tokio::time::sleep_until(last_activity + timeout) => break,
                    payload = up_rx.recv() => {
                        let Some(payload) = payload else { break; };
                        if payload.is_empty() {
                            break;
                        }
                        last_activity = TokioInstant::now();
                        let request = match udp_session.decode_mux_dispatch_parts(&payload) {
                            Ok(request) => request,
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp packet parse failed");
                                break;
                            }
                        };
                        if let Err(error) = UdpPipe::new(&proxy, &mut dispatch)
                            .dispatch(UdpPipeInput {
                                target: request.target,
                                port: request.port,
                                payload: &request.payload,
                                protocol: ProtocolType::Vmess,
                                auth: None,
                                client_session_id: request.client_session_id,
                            })
                            .await
                        {
                            warn!(%error, mux_session_id, "vmess mux udp packet dispatch failed");
                        }
                    }
                    recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                        match recv {
                            Ok((n, sender)) => {
                                last_activity = TokioInstant::now();
                                let session_id = dispatch.direct_response_session_id(sender);
                                record_udp_inbound_response_rx(&proxy, session_id, n);
                                match udp_session.write_mux_response_to_socket_addr(
                                    &writer,
                                    mux_session_id,
                                    sender,
                                    &direct_buf[..n],
                                ) {
                                    Ok(frame_len) => {
                                        record_udp_inbound_response_tx(&proxy, session_id, frame_len);
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp direct response send failed");
                                        break;
                                    }
                                }
                            }
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp direct recv failed");
                                break;
                            }
                        }
                    }
                    upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                        match upstream {
                            Ok(pkt) => {
                                last_activity = TokioInstant::now();
                                proxy.record_udp_upstream_packet_received();
                                dispatch.touch_upstream_idle(proxy.udp_upstream_idle_timeout());
                                let (target, port, payload) = pkt.into_parts();
                                let session_id = udp_response_session_id(&dispatch, &target, port);
                                record_udp_inbound_response_rx(&proxy, session_id, payload.len());
                                match udp_session.write_mux_response(
                                    &writer,
                                    mux_session_id,
                                    &target,
                                    port,
                                    &payload,
                                ) {
                                    Ok(frame_len) => {
                                        record_udp_inbound_response_tx(&proxy, session_id, frame_len);
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp upstream response send failed");
                                        break;
                                    }
                                }
                            }
                            Err(error) => warn!(%error, mux_session_id, "vmess mux udp socks5 upstream recv failed"),
                        }
                    }
                    _ = wait_for_upstream_idle(socks5_idle) => {}
                    Some(chain_result) = chain_tasks.join_next() => {
                        match chain_result {
                            Ok(Ok((target, port, payload, session_id))) => {
                                last_activity = TokioInstant::now();
                                record_udp_inbound_response_rx(&proxy, session_id, payload.len());
                                match udp_session.write_mux_response(
                                    &writer,
                                    mux_session_id,
                                    &target,
                                    port,
                                    &payload,
                                ) {
                                    Ok(frame_len) => {
                                        record_udp_inbound_response_tx(&proxy, session_id, frame_len);
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp chain response send failed");
                                        break;
                                    }
                                }
                            }
                            Ok(Err(error)) => warn!(%error, mux_session_id, "vmess mux udp chain response failed"),
                            Err(error) => warn!(%error, mux_session_id, "vmess mux udp chain task panicked"),
                        }
                    }
                }
            }

            for completed in dispatch.finish_all() {
                log_completed_udp_flow(completed);
            }
            let _ = mux_session.end_inbound_stream(&writer, mux_session_id);
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
        let mut udp_session = vmess::VmessInbound.udp_session(session.target.clone(), session.port);
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
                read = udp_session.read_dispatch_parts_tokio(&mut client, &mut client_buf) => {
                    match read {
                        Ok(None) => break,
                        Ok(Some(request)) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput {
                                    target: request.target,
                                    port: request.port,
                                    payload: &request.payload,
                                    protocol: ProtocolType::Vmess,
                                    auth: auth.as_ref(),
                                    client_session_id: request.client_session_id,
                                })
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
                    let session_id = dispatch.direct_response_session_id(sender);
                    record_udp_inbound_response_rx(self, session_id, n);
                    let written = udp_session.write_response_to_socket_addr_tokio(
                        &mut client,
                        sender,
                        &direct_buf[..n],
                    ).await?;
                    record_udp_inbound_response_tx(self, session_id, written);
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(self.udp_upstream_idle_timeout());
                            let (target, port, payload) = pkt.into_parts();
                            let session_id = udp_response_session_id(&dispatch, &target, port);
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await?;
                            record_udp_inbound_response_tx(self, session_id, written);
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
                            record_udp_inbound_response_rx(self, session_id, payload.len());
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await?;
                            record_udp_inbound_response_tx(self, session_id, written);
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
