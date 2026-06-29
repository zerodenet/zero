//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::{Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{log_completed_udp_flow, wait_for_upstream_idle};
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
        let mut mux_tasks: JoinSet<()> = JoinSet::new();
        let mux_session = vmess::VmessInboundMuxSession::new();
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
                event = mux_session.next_event(&mut reader) => {
                    let event = match event {
                        Ok(event) => event,
                        Err(error) => {
                            warn!(error = %error, "vmess mux frame read failed");
                            break;
                        }
                    };

                    match event {
                        vmess::VmessMuxServerEvent::KeepAlive => continue,
                        vmess::VmessMuxServerEvent::NewStream {
                            session_id,
                            network,
                            target,
                            port,
                            payload,
                        } => {
                            let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                            streams.insert(session_id, up_tx.clone());
                            if !payload.is_empty() {
                                let _ = up_tx.send(payload);
                            }
                            match network {
                                Network::Tcp => {
                                    self.spawn_vmess_mux_tcp_stream_task(VmessMuxTcpStreamTask {
                                        tasks: &mut mux_tasks,
                                        mux_session_id: session_id,
                                        target,
                                        port,
                                        up_rx,
                                        write_tx: write_tx.clone(),
                                        inbound_tag: inbound_tag.to_owned(),
                                    })
                                }
                                Network::Udp => {
                                    self.spawn_vmess_mux_udp_stream_task(VmessMuxUdpStreamTask {
                                        tasks: &mut mux_tasks,
                                        mux_session_id: session_id,
                                        default_target: target,
                                        default_port: port,
                                        up_rx,
                                        write_tx: write_tx.clone(),
                                        inbound_tag: inbound_tag.to_owned(),
                                    })
                                }
                            }
                        }
                        vmess::VmessMuxServerEvent::Data {
                            session_id,
                            payload,
                        } => {
                            if let Some(tx) = streams.get(&session_id) {
                                if !payload.is_empty() {
                                    let _ = tx.send(payload);
                                }
                            }
                        }
                        vmess::VmessMuxServerEvent::End { session_id } => {
                            if let Some(tx) = streams.remove(&session_id) {
                                let _ = tx.send(Vec::new());
                            }
                        }
                        vmess::VmessMuxServerEvent::Unknown { .. } => {}
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
            write_tx,
            inbound_tag,
        } = request;
        let mut up_rx = up_rx;
        let proxy = self.clone();
        tasks.spawn(async move {
            let mut session =
                Session::new(0, target, port, Network::Tcp, ProtocolType::Vmess);
            proxy.prepare_session(&mut session, &inbound_tag, None);

            let route = match proxy.dispatch_tcp(&mut session).await {
                Ok(route) => route,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux dispatch failed");
                    let _ = vmess::VmessInboundMuxSession::new().queue_end(&write_tx, mux_session_id);
                    return;
                }
            };

            let mut upstream = route.upstream;
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
                                match vmess::VmessInboundMuxSession::new().queue_data(&write_tx, mux_session_id, &buf[..n]) {
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
            let _ = vmess::VmessInboundMuxSession::new().queue_end(&write_tx, mux_session_id);
        });
    }

    pub(crate) fn spawn_vmess_mux_udp_stream_task(&self, request: VmessMuxUdpStreamTask<'_>) {
        let VmessMuxUdpStreamTask {
            tasks,
            mux_session_id,
            default_target,
            default_port,
            up_rx,
            write_tx,
            inbound_tag,
        } = request;
        let mut up_rx = up_rx;
        let proxy = self.clone();
        tasks.spawn(async move {
            let mut udp_session =
                vmess::VmessInbound.udp_session(default_target, default_port);
            let mut dispatch = match UdpDispatch::new(&inbound_tag).await {
                Ok(dispatch) => dispatch,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux udp dispatch init failed");
                    let _ = vmess::VmessInboundMuxSession::new().queue_end(&write_tx, mux_session_id);
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
                        let request = match udp_session.decode_request(&payload) {
                            Ok(request) => request,
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp packet parse failed");
                                break;
                            }
                        };
                        let request = request.into_dispatch_parts();
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
                                let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                                if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                    proxy.record_session_outbound_rx(sid, n as u64);
                                }
                                match udp_session.send_mux_response_to_ip(
                                    &write_tx,
                                    mux_session_id,
                                    ip,
                                    sender.port(),
                                    &direct_buf[..n],
                                ) {
                                    Ok(frame_len) => {
                                        if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                            proxy.record_session_inbound_tx(sid, frame_len as u64);
                                        }
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
                                if let Some(sid) = dispatch.session_id_by_target(&target, port, None) {
                                    proxy.record_session_outbound_rx(sid, payload.len() as u64);
                                }
                                match udp_session.send_mux_response(
                                    &write_tx,
                                    mux_session_id,
                                    &target,
                                    port,
                                    &payload,
                                ) {
                                    Ok(frame_len) => {
                                        if let Some(sid) = dispatch.session_id_by_target(&target, port, None) {
                                            proxy.record_session_inbound_tx(sid, frame_len as u64);
                                        }
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
                                if let Some(sid) = session_id {
                                    proxy.record_session_outbound_rx(sid, payload.len() as u64);
                                }
                                match udp_session.send_mux_response(
                                    &write_tx,
                                    mux_session_id,
                                    &target,
                                    port,
                                    &payload,
                                ) {
                                    Ok(frame_len) => {
                                        if let Some(sid) = session_id {
                                            proxy.record_session_inbound_tx(sid, frame_len as u64);
                                        }
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
            let _ = vmess::VmessInboundMuxSession::new().queue_end(&write_tx, mux_session_id);
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
                read = client.read(&mut client_buf) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            last_activity = TokioInstant::now();
                            let request = match udp_session.decode_request(&client_buf[..n]) {
                                Ok(request) => request,
                                Err(error) => {
                                    warn!(error = %error, "vmess udp client packet parse error");
                                    break;
                                }
                            };
                            let request = request.into_dispatch_parts();
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
                            warn!(error = %error, "vmess udp client read error");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();
                    let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                    let session_id = dispatch.direct_response_session_id(sender);
                    if let Some(sid) = session_id {
                        self.record_session_outbound_rx(sid, n as u64);
                    }
                    let written = udp_session.write_response_to_ip_tokio(
                        &mut client,
                        ip,
                        sender.port(),
                        &direct_buf[..n],
                    ).await?;
                    if let Some(sid) = session_id {
                        self.record_session_inbound_tx(sid, written as u64);
                    }
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_upstream_idle(self.udp_upstream_idle_timeout());
                            let (target, port, payload) = pkt.into_parts();
                            if let Some(sid) = dispatch.session_id_by_target(&target, port, None) {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await?;
                            if let Some(sid) = dispatch.session_id_by_target(&target, port, None) {
                                self.record_session_inbound_tx(sid, written as u64);
                            }
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
                            if let Some(sid) = session_id {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            let written = udp_session.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await?;
                            if let Some(sid) = session_id {
                                self.record_session_inbound_tx(sid, written as u64);
                            }
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
