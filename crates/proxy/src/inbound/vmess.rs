//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use vmess::{VmessAccept, VmessAeadStream, VmessCipher, VmessInbound, VmessUser};
use zero_config::{GrpcConfig, InboundConfig, WebSocketConfig};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_associate::helpers::{
    log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.
struct TlsStream(tokio_rustls::server::TlsStream<TcpRelayStream>);

impl AsyncSocket for TlsStream {
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        tokio::io::AsyncReadExt::read(&mut self.0, buf).await
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(&mut self.0, buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::shutdown(&mut self.0).await
    }
}

// Trait-based handler (raw TLS path).

#[derive(Clone)]
pub(crate) struct VmessInboundHandler {
    vmess_inbound: VmessInbound,
    users: Vec<VmessUser>,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for VmessInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(e)))?;
        let mut sock = TlsStream(tls);
        let accepted = if self.users.len() == 1 {
            self.vmess_inbound
                .accept_tcp(&mut sock, &self.users[0])
                .await?
        } else {
            self.vmess_inbound
                .accept_tcp_multi(&mut sock, &self.users)
                .await?
        };
        let session = accepted.session.clone();
        let client = wrap_vmess_client(TcpRelayStream::new(sock.0), accepted)?;
        Ok((session, client))
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// Handler for transport-wrapped connections (WS/gRPC).
// Only send_ok / send_blocked / send_upstream_failure are used by serve_inbound;
// accept is unreachable because the protocol was already authenticated.

#[derive(Clone)]
struct VmessTransportHandler;

#[async_trait]
impl InboundProtocol for VmessTransportHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        unreachable!("accept handled in listener transport dispatch")
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// Listener.

impl Proxy {
    pub(crate) async fn run_vmess_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (users, tls_cfg, ws_config, grpc_config) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Vmess {
                users,
                tls,
                ws,
                grpc,
            } => (users.clone(), tls.clone(), ws.clone(), grpc.clone()),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "vmess config",
                )))
            }
        };
        if users.is_empty() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess requires at least one user",
            )));
        }

        let vmess_users: Vec<VmessUser> = users
            .iter()
            .map(|u| {
                let uuid = vmess::parse_uuid(&u.id)
                    .map_err(|e| EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, e)))?;
                let cipher = VmessCipher::from_name(&u.cipher).ok_or_else(|| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("vmess unknown cipher: {}", u.cipher),
                    ))
                })?;
                Ok(VmessUser {
                    id: uuid,
                    cipher,
                    credential_id: u.credential_id.clone(),
                    principal_key: u.principal_key.clone(),
                    up_bps: u.up_bps,
                    down_bps: u.down_bps,
                })
            })
            .collect::<Result<Vec<_>, EngineError>>()?;

        let tls_cfg = tls_cfg.ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess requires TLS",
            ))
        })?;
        let acceptor = crate::transport::build_tls_acceptor(&tls_cfg, self.config.source_dir())?;
        let listener = bind_listener(&inbound).await?;
        let tag = inbound.tag.clone();

        let handler = VmessInboundHandler {
            vmess_inbound: VmessInbound,
            users: vmess_users,
            tls_acceptor: acceptor,
        };

        let transport = match (&ws_config, &grpc_config) {
            (Some(_), Some(_)) => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "vmess: ws and grpc are mutually exclusive",
                )))
            }
            (Some(_), None) => "vmess+ws",
            (None, Some(_)) => "vmess+grpc",
            (None, None) => "vmess",
        };

        info!(inbound_tag = %tag, protocol = transport, listen = %listener.local_addr()?, "started");

        let mut conns = JoinSet::new();
        loop {
            select! {
                _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
                r = listener.accept() => {
                    let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                    let p = self.clone();
                    let t = tag.clone();
                    let h = handler.clone();
                    let ws = ws_config.clone();
                    let grpc = grpc_config.clone();
                    let source_addr = remote_addr_to_socket(peer);
                    conns.spawn(async move {
                        let res = if let Some(grpc_cfg) = &grpc {
                            handle_vmess_grpc(&p, &h, s.into(), grpc_cfg, &t, source_addr).await
                        } else if let Some(ws_cfg) = &ws {
                            handle_vmess_ws(&p, &h, s.into(), ws_cfg, &t, source_addr).await
                        } else {
                            handle_vmess_raw(&p, &h, s.into(), &t, source_addr).await
                        };
                        if let Err(e) = res {
                            if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                            { warn!(?source_addr, %e, "vmess failed"); }
                        }
                    });
                }
                r = conns.join_next(), if !conns.is_empty() => {
                    if let Some(Err(e)) = r { if !e.is_cancelled() { error!(%e, "task panicked"); } }
                }
            }
        }
        conns.abort_all();
        info!(inbound_tag = %tag, "stopped");
        Ok(())
    }
}

/// Raw TLS path: TLS accept -> VMess auth -> serve_inbound.
async fn handle_vmess_raw(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    match handler.accept(stream).await {
        Ok((session, client)) => {
            if session.network == Network::Udp {
                proxy.run_vmess_udp_relay(client, session, tag).await
            } else if vmess::is_mux_cool_session(&session) {
                proxy.run_vmess_mux_session(client, tag).await
            } else {
                serve_inbound(proxy, session, client, handler, tag, source_addr).await
            }
        }
        Err(e) => Err(e),
    }
}

/// WebSocket path: TLS accept -> WS upgrade -> VMess auth -> serve_inbound.
async fn handle_vmess_ws(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    ws_cfg: &WebSocketConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let mut ws = crate::transport::accept_ws(tls, &ws_cfg.path).await?;

    let accepted = if handler.users.len() == 1 {
        handler
            .vmess_inbound
            .accept_tcp(&mut ws, &handler.users[0])
            .await?
    } else {
        handler
            .vmess_inbound
            .accept_tcp_multi(&mut ws, &handler.users)
            .await?
    };
    let session = accepted.session.clone();
    let client = wrap_vmess_client(TcpRelayStream::new(ws), accepted)?;

    let transport_handler = VmessTransportHandler;
    if session.network == Network::Udp {
        proxy.run_vmess_udp_relay(client, session, tag).await
    } else if vmess::is_mux_cool_session(&session) {
        proxy.run_vmess_mux_session(client, tag).await
    } else {
        serve_inbound(proxy, session, client, &transport_handler, tag, source_addr).await
    }
}

/// gRPC path: TLS accept -> serve_grpc -> per-stream VMess auth -> serve_inbound.
async fn handle_vmess_grpc(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    grpc_cfg: &GrpcConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let service_names = grpc_cfg.service_names.clone();
    let users = handler.users.clone();
    let vmess = handler.vmess_inbound;
    let proxy = proxy.clone();
    let tag = tag.to_owned();

    crate::transport::serve_grpc(tls, &service_names, move |mut grpc_stream| {
        let users = users.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = if users.len() == 1 {
                vmess.accept_tcp(&mut grpc_stream, &users[0]).await
            } else {
                vmess.accept_tcp_multi(&mut grpc_stream, &users).await
            };
            match result {
                Ok(accepted) => {
                    let session = accepted.session.clone();
                    let client = wrap_vmess_client(TcpRelayStream::new(grpc_stream), accepted)?;
                    let transport_handler = VmessTransportHandler;
                    if session.network == Network::Udp {
                        proxy.run_vmess_udp_relay(client, session, &tag).await
                    } else if vmess::is_mux_cool_session(&session) {
                        proxy.run_vmess_mux_session(client, &tag).await
                    } else {
                        serve_inbound(
                            &proxy,
                            session,
                            client,
                            &transport_handler,
                            &tag,
                            source_addr,
                        )
                        .await
                    }
                }
                Err(e) => {
                    warn!(%e, "vmess grpc auth failed");
                    Err(EngineError::Core(zero_core::Error::Protocol(
                        "vmess auth failed",
                    )))
                }
            }
        }
    })
    .await
}

impl Proxy {
    async fn run_vmess_mux_session(
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
                frame = read_vmess_mux_frame_from_tokio(&mut reader) => {
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
                            Network::Tcp => self.spawn_vmess_mux_tcp_stream_task(
                                &mut mux_tasks,
                                frame.session_id,
                                target,
                                port,
                                up_rx,
                                write_tx.clone(),
                                inbound_tag.to_owned(),
                            ),
                            Network::Udp => self.spawn_vmess_mux_udp_stream_task(
                                &mut mux_tasks,
                                frame.session_id,
                                target,
                                port,
                                up_rx,
                                write_tx.clone(),
                                inbound_tag.to_owned(),
                            ),
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

    fn spawn_vmess_mux_tcp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        target: Address,
        port: u16,
        mut up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        inbound_tag: String,
    ) {
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

    fn spawn_vmess_mux_udp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        default_target: Address,
        default_port: u16,
        mut up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        inbound_tag: String,
    ) {
        let proxy = self.clone();
        tasks.spawn(async move {
            let mut payload_mode = VmessUdpPayloadMode::Unknown;
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
                        let input = match payload_mode {
                            VmessUdpPayloadMode::Unknown => match vmess::parse_udp_packet(&payload) {
                                Ok(packet) => {
                                    payload_mode = VmessUdpPayloadMode::VmessPacket;
                                    (packet.target, packet.port, packet.payload)
                                }
                                Err(_) => {
                                    payload_mode = VmessUdpPayloadMode::RawDatagram;
                                    (default_target.clone(), default_port, payload)
                                }
                            },
                            VmessUdpPayloadMode::VmessPacket => match vmess::parse_udp_packet(&payload) {
                                Ok(packet) => (packet.target, packet.port, packet.payload),
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vmess mux udp packet parse failed");
                                    break;
                                }
                            },
                            VmessUdpPayloadMode::RawDatagram => {
                                (default_target.clone(), default_port, payload)
                            }
                        };
                        if let Err(error) = UdpPipe::new(&proxy, &mut dispatch)
                            .dispatch(UdpPipeInput {
                                target: input.0,
                                port: input.1,
                                payload: &input.2,
                                protocol: ProtocolType::Vmess,
                                auth: None,
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
                                if let Ok(pkt) = socks5::parse_udp_packet(&upstream_buf[..read]) {
                                    if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port) {
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
                                            if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port) {
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

    async fn run_vmess_udp_relay(
        &self,
        mut client: TcpRelayStream,
        session: Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let default_target = session.target.clone();
        let default_port = session.port;
        let mut payload_mode = VmessUdpPayloadMode::Unknown;
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
                            let input = match payload_mode {
                                VmessUdpPayloadMode::Unknown => match vmess::parse_udp_packet(&client_buf[..n]) {
                                    Ok(packet) => {
                                        payload_mode = VmessUdpPayloadMode::VmessPacket;
                                        (packet.target, packet.port, packet.payload)
                                    }
                                    Err(_) => {
                                        payload_mode = VmessUdpPayloadMode::RawDatagram;
                                        (default_target.clone(), default_port, client_buf[..n].to_vec())
                                    }
                                },
                                VmessUdpPayloadMode::VmessPacket => match vmess::parse_udp_packet(&client_buf[..n]) {
                                    Ok(packet) => (packet.target, packet.port, packet.payload),
                                    Err(error) => {
                                        warn!(error = %error, "vmess udp client packet parse error");
                                        break;
                                    }
                                },
                                VmessUdpPayloadMode::RawDatagram => {
                                    (default_target.clone(), default_port, client_buf[..n].to_vec())
                                }
                            };
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput {
                                    target: input.0,
                                    port: input.1,
                                    payload: &input.2,
                                    protocol: ProtocolType::Vmess,
                                    auth: auth.as_ref(),
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
                            if let Ok(pkt) = socks5::parse_udp_packet(&upstream_buf[..read]) {
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port) {
                                    self.record_session_outbound_rx(sid, pkt.payload.len() as u64);
                                }
                                let packet = encode_vmess_udp_response(
                                    payload_mode,
                                    &pkt.target,
                                    pkt.port,
                                    &pkt.payload,
                                )?;
                                client.write_all(&packet).await?;
                                if let Some(sid) = dispatch.session_id_by_target(&pkt.target, pkt.port) {
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

#[derive(Clone, Copy)]
enum VmessUdpPayloadMode {
    Unknown,
    VmessPacket,
    RawDatagram,
}

fn encode_vmess_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    match mode {
        VmessUdpPayloadMode::Unknown | VmessUdpPayloadMode::VmessPacket => {
            let packet = vmess::build_udp_packet(target, port, payload)?;
            vmess::encode_mux_keep_stream(mux_session_id, &packet)
        }
        VmessUdpPayloadMode::RawDatagram => vmess::encode_mux_keep_stream(mux_session_id, payload),
    }
}

fn encode_vmess_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    match mode {
        VmessUdpPayloadMode::Unknown | VmessUdpPayloadMode::VmessPacket => {
            vmess::build_udp_packet(target, port, payload)
        }
        VmessUdpPayloadMode::RawDatagram => Ok(payload.to_vec()),
    }
}

fn wrap_vmess_client(
    stream: TcpRelayStream,
    accepted: VmessAccept,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(VmessAeadStream::inbound(
        stream, accepted,
    )?))
}

async fn read_vmess_mux_frame_from_tokio<R>(reader: &mut R) -> Result<vmess::MuxFrame, EngineError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut len_buf = [0_u8; 2];
    tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf).await?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > vmess::MUX_MAX_META_LEN {
        return Err(EngineError::Core(zero_core::Error::Protocol(
            "vmess mux metadata too large",
        )));
    }
    let mut meta = vec![0_u8; meta_len];
    tokio::io::AsyncReadExt::read_exact(reader, &mut meta).await?;
    let mut frame = vmess::decode_mux_metadata(&meta)?;
    if frame.option & vmess::MUX_OPTION_DATA != 0 {
        tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf).await?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > vmess::MUX_MAX_DATA_LEN {
            return Err(EngineError::Core(zero_core::Error::Protocol(
                "vmess mux data too large",
            )));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            tokio::io::AsyncReadExt::read_exact(reader, &mut frame.payload).await?;
        }
    }
    Ok(frame)
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
