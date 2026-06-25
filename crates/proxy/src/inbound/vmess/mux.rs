//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::protocol_runtime::socks5_udp::recv_upstream_packet;
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{log_completed_udp_flow, wait_for_upstream_idle};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

use super::model::{VmessMuxTcpStreamTask, VmessMuxUdpStreamTask};
use super::{
    decode_vmess_udp_payload, encode_vmess_mux_udp_response, encode_vmess_udp_response,
    VmessUdpPayloadMode,
};

impl Proxy {
    pub(crate) async fn run_vmess_mux_session(
        &self,
        client: TcpRelayStream,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let (mut reader, mut writer) = tokio::io::split(client);
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut mux_tasks: JoinSet<()> = JoinSet::new();
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
                frame = vmess::read_mux_frame_from_tokio(&mut reader) => {
                    let frame = match frame {
                        Ok(frame) => frame,
                        Err(error) => {
                            warn!(error = %error, "vmess mux frame read failed");
                            break;
                        }
                    };

                    if frame.status == vmess::MUX_STATUS_KEEP_ALIVE {
                        continue;
                    }

                    if frame.status == vmess::MUX_STATUS_NEW {
                        let Some(network) = frame.network else {
                            warn!("vmess mux new frame missing network");
                            continue;
                        };
                        let (Some(target), Some(port)) = (frame.target.clone(), frame.port) else {
                            warn!("vmess mux new frame missing target");
                            let _ = write_tx.send(vmess::encode_mux_end_stream(frame.session_id)?);
                            continue;
                        };

                        let (up_tx, up_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                        streams.insert(frame.session_id, up_tx.clone());
                        if !frame.payload.is_empty() {
                            let _ = up_tx.send(frame.payload);
                        }
                        match network {
                            Network::Tcp => {
                                self.spawn_vmess_mux_tcp_stream_task(VmessMuxTcpStreamTask {
                                    tasks: &mut mux_tasks,
                                    mux_session_id: frame.session_id,
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
                                    mux_session_id: frame.session_id,
                                    default_target: target,
                                    default_port: port,
                                    up_rx,
                                    write_tx: write_tx.clone(),
                                    inbound_tag: inbound_tag.to_owned(),
                                })
                            }
                        }
                    } else if frame.status == vmess::MUX_STATUS_KEEP {
                        if let Some(tx) = streams.get(&frame.session_id) {
                            if !frame.payload.is_empty() {
                                let _ = tx.send(frame.payload);
                            }
                        }
                    } else if frame.status == vmess::MUX_STATUS_END {
                        if let Some(tx) = streams.remove(&frame.session_id) {
                            let _ = tx.send(Vec::new());
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
                    let _ = write_tx.send(
                        vmess::encode_mux_end_stream(mux_session_id).unwrap_or_default(),
                    );
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
                                match vmess::encode_mux_keep_stream(mux_session_id, &buf[..n]) {
                                    Ok(frame) => {
                                        if write_tx.send(frame).is_err() {
                                            break;
                                        }
                                    }
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
            let _ = write_tx.send(vmess::encode_mux_end_stream(mux_session_id).unwrap_or_default());
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
            let mut payload_mode = VmessUdpPayloadMode::unknown();
            let mut dispatch = match UdpDispatch::new(&inbound_tag).await {
                Ok(dispatch) => dispatch,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux udp dispatch init failed");
                    let _ = write_tx.send(
                        vmess::encode_mux_end_stream(mux_session_id).unwrap_or_default(),
                    );
                    return;
                }
            };
            let timeout = proxy.udp_upstream_idle_timeout();
            let mut last_activity = TokioInstant::now();
            let mut direct_buf = vec![0_u8; 64 * 1024];
            let mut upstream_buf = vec![0_u8; 64 * 1024];

            loop {
                let (direct_sock, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();
                select! {
                    _ = tokio::time::sleep_until(last_activity + timeout) => break,
                    payload = up_rx.recv() => {
                        let Some(payload) = payload else { break; };
                        if payload.is_empty() {
                            break;
                        }
                        last_activity = TokioInstant::now();
                        let input = match decode_vmess_udp_payload(
                            &mut payload_mode,
                            &default_target,
                            default_port,
                            &payload,
                        ) {
                            Ok(input) => input,
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp packet parse failed");
                                break;
                            }
                        };
                        if let Err(error) = UdpPipe::new(&proxy, &mut dispatch)
                            .dispatch(UdpPipeInput {
                                target: input.target,
                                port: input.port,
                                payload: &input.payload,
                                protocol: ProtocolType::Vmess,
                                auth: None,
                                client_session_id: None,
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
                                let target = match zero_platform_tokio::socket_addr_to_ip(sender) {
                                    zero_traits::IpAddress::V4(bytes) => Address::Ipv4(bytes),
                                    zero_traits::IpAddress::V6(bytes) => Address::Ipv6(bytes),
                                };
                                if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                    proxy.record_session_outbound_rx(sid, n as u64);
                                }
                                let encoded = encode_vmess_mux_udp_response(
                                    mux_session_id,
                                    payload_mode,
                                    &target,
                                    sender.port(),
                                    &direct_buf[..n],
                                );
                                match encoded {
                                    Ok(frame) => {
                                        let frame_len = frame.len() as u64;
                                        if write_tx.send(frame).is_err() {
                                            break;
                                        }
                                        if let Some(sid) = dispatch.direct_response_session_id(sender) {
                                            proxy.record_session_inbound_tx(sid, frame_len);
                                        }
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp direct response encode failed");
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
                    upstream = recv_upstream_packet(socks5_up, &mut upstream_buf) => {
                        match upstream {
                            Ok(read) => {
                                last_activity = TokioInstant::now();
                                proxy.record_udp_upstream_packet_received();
                                dispatch.touch_socks5_idle(proxy.udp_upstream_idle_timeout());
                                if let Ok(pkt) = socks5::decode_udp_associate_response(&upstream_buf[..read]) {
                                    if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                        proxy.record_session_outbound_rx(sid, pkt.payload.len() as u64);
                                    }
                                    match encode_vmess_mux_udp_response(
                                        mux_session_id,
                                        payload_mode,
                                        &pkt.target,
                                        pkt.port,
                                        &pkt.payload,
                                    ) {
                                        Ok(frame) => {
                                            let frame_len = frame.len() as u64;
                                            if write_tx.send(frame).is_err() {
                                                break;
                                            }
                                            if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                                proxy.record_session_inbound_tx(sid, frame_len);
                                            }
                                        }
                                        Err(error) => {
                                            warn!(%error, mux_session_id, "vmess mux udp upstream response encode failed");
                                            break;
                                        }
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
                                match encode_vmess_mux_udp_response(
                                    mux_session_id,
                                    payload_mode,
                                    &target,
                                    port,
                                    &payload,
                                ) {
                                    Ok(frame) => {
                                        let frame_len = frame.len() as u64;
                                        if write_tx.send(frame).is_err() {
                                            break;
                                        }
                                        if let Some(sid) = session_id {
                                            proxy.record_session_inbound_tx(sid, frame_len);
                                        }
                                    }
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp chain response encode failed");
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
            let _ = write_tx.send(vmess::encode_mux_end_stream(mux_session_id).unwrap_or_default());
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
        let default_target = session.target.clone();
        let default_port = session.port;
        let mut payload_mode = VmessUdpPayloadMode::unknown();
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
            let (direct_sock, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();

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
                            let input = match decode_vmess_udp_payload(
                                &mut payload_mode,
                                &default_target,
                                default_port,
                                &client_buf[..n],
                            ) {
                                Ok(input) => input,
                                Err(error) => {
                                    warn!(error = %error, "vmess udp client packet parse error");
                                    break;
                                }
                            };
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput {
                                    target: input.target,
                                    port: input.port,
                                    payload: &input.payload,
                                    protocol: ProtocolType::Vmess,
                                    auth: auth.as_ref(),
                                    client_session_id: None,
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
                    let target = match zero_platform_tokio::socket_addr_to_ip(sender) {
                        zero_traits::IpAddress::V4(bytes) => Address::Ipv4(bytes),
                        zero_traits::IpAddress::V6(bytes) => Address::Ipv6(bytes),
                    };
                    let session_id = dispatch.direct_response_session_id(sender);
                    if let Some(sid) = session_id {
                        self.record_session_outbound_rx(sid, n as u64);
                    }
                    let packet = encode_vmess_udp_response(
                        payload_mode,
                        &target,
                        sender.port(),
                        &direct_buf[..n],
                    )?;
                    client.write_all(&packet).await?;
                    if let Some(sid) = session_id {
                        self.record_session_inbound_tx(sid, packet.len() as u64);
                    }
                }
                upstream = recv_upstream_packet(socks5_up, &mut upstream_buf) => {
                    match upstream {
                        Ok(read) => {
                            last_activity = TokioInstant::now();
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_socks5_idle(self.udp_upstream_idle_timeout());
                            if let Ok(pkt) = socks5::decode_udp_associate_response(&upstream_buf[..read]) {
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                    self.record_session_outbound_rx(sid, pkt.payload.len() as u64);
                                }
                                let packet = encode_vmess_udp_response(
                                    payload_mode,
                                    &pkt.target,
                                    pkt.port,
                                    &pkt.payload,
                                )?;
                                client.write_all(&packet).await?;
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port, None) {
                                    self.record_session_inbound_tx(sid, packet.len() as u64);
                                }
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
                            let packet = encode_vmess_udp_response(
                                payload_mode,
                                &target,
                                port,
                                &payload,
                            )?;
                            client.write_all(&packet).await?;
                            if let Some(sid) = session_id {
                                self.record_session_inbound_tx(sid, packet.len() as u64);
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
