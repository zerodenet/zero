use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use vless::build_udp_packet;
use vless::RealityServerOptions;
use vless::{VlessUser, VlessUserStore};
use zero_config::{InboundRealityConfig, VlessUserConfig};
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

use crate::runtime::udp_associate::helpers::{
    log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use crate::runtime::udp_dispatch::UdpDispatch;

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::{accept_ws, build_tls_acceptor, InboundTlsStream, PrefixedSocket};
use crate::transport::{
    relay_bidirectional_metered, ClientStream, EstablishedTcpOutbound, MeteredStream,
    TcpRelayStream,
};
use async_trait::async_trait;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};

// ── Handler (TCP path only) ─────────────────────────────────────────────

#[derive(Clone)]
struct VlessInboundHandler {
    vless_inbound: vless::VlessInbound,
}

#[async_trait]
impl InboundProtocol for VlessInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(zero_core::Session, Self::ClientStream), EngineError> {
        // VLESS accept is handled inline by the listener (complex dispatch).
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "VLESS accept handled by listener",
        )))
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.vless_inbound
            .send_response(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = AsyncSocket::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = AsyncSocket::shutdown(client).await;
        Ok(())
    }
    // relay uses default
}

impl Proxy {
    pub(crate) async fn run_vless_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listen_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        let quic_config = inbound.protocol.vless_quic().cloned();

        // QUIC uses UDP — if configured, create a QUIC endpoint instead of TCP
        if let Some(ref quic) = quic_config {
            if let (Some(cert_path), Some(key_path)) = (&quic.cert_path, &quic.key_path) {
                let quic_inbound = crate::transport::QuicInbound::bind(
                    &listen_addr.to_string(),
                    cert_path,
                    key_path,
                    self.config.source_dir(),
                )
                .await?;

                info!(
                    inbound_tag = %inbound.tag,
                    protocol = "vless",
                    listen = %listen_addr,
                    transport = "quic",
                    "inbound listener ready"
                );

                let mut connections = JoinSet::new();
                let fallback_config = inbound.protocol.vless_fallback().cloned();
                return Self::run_vless_quic_accept_loop(
                    self,
                    &inbound,
                    &quic_inbound,
                    &mut shutdown,
                    &mut connections,
                    fallback_config,
                )
                .await;
            }
        }

        // Standard TCP listener path
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let tls_acceptor = inbound
            .protocol
            .vless_tls()
            .map(|tls| build_tls_acceptor(tls, self.config.source_dir()))
            .transpose()?;
        let reality_config = inbound.protocol.vless_reality().cloned();
        let ws_config = inbound.protocol.vless_ws().cloned();
        let grpc_config = inbound.protocol.vless_grpc().cloned();
        let h2_config = inbound.protocol.vless_h2().cloned();
        let http_upgrade_config = inbound.protocol.vless_http_upgrade().cloned();
        let split_http_config = inbound.protocol.vless_split_http().cloned();
        let split_http_registry: Option<crate::transport::SplitHttpRegistry> = split_http_config
            .as_ref()
            .map(|_| crate::transport::SplitHttpRegistry::new());
        let fallback_config = inbound.protocol.vless_fallback().cloned();
        let vless_users: Arc<[zero_config::VlessUserConfig]> =
            inbound.protocol.vless_users().into();
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            listen = %local_addr,
            tls = tls_acceptor.is_some(),
            reality = reality_config.is_some(),
            ws = ws_config.is_some(),
            grpc = grpc_config.is_some(),
            http_upgrade = http_upgrade_config.is_some(),
            fallback = fallback_config.is_some(),
            "inbound listener ready"
        );

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = listener.accept() => {
                    let (stream, remote_addr) = accept_result?;
                    let engine = self.clone();
                    let inbound_tag = inbound.tag.clone();
                    let vless_users = Arc::clone(&vless_users);
                    let tls_acceptor = tls_acceptor.clone();
                    let reality_config = reality_config.clone();
                    let ws_config = ws_config.clone();
                    let grpc_config = grpc_config.clone();
                    let h2_config = h2_config.clone();
                    let http_upgrade_config = http_upgrade_config.clone();
                    let split_http_config = split_http_config.clone();
                    let split_http_registry = split_http_registry.clone();
                    let fallback_config = fallback_config.clone();

                    connections.spawn(async move {
                        let result = match (tls_acceptor, reality_config) {
                            (Some(acceptor), None) => {
                                // Always peek ClientHello to extract SNI for routing.
                                // Also used for ALPN-based fallback when configured.
                                let mut raw = stream.into_inner();
                                let hello = crate::transport::tls_hello::peek_client_hello(
                                    &mut raw,
                                ).await.ok();

                                if let Some(hello) = hello {
                                    // Check ALPN fallback match
                                    let alpn_match = fallback_config.as_ref()
                                        .and_then(|fb| fb.alpn.as_ref().zip(Some(fb)))
                                        .and_then(|(expected, fb)| {
                                            hello.alpn.iter()
                                                .find(|a| *a == expected)
                                                .map(|_| fb)
                                        });

                                    if let Some(fb) = alpn_match {
                                        let mut upstream = engine.protocols.direct_outbound
                                            .connect_host(&fb.server, fb.port, &engine.resolver)
                                            .await?;
                                        tokio::io::AsyncWriteExt::write_all(
                                            &mut upstream, &hello.consumed,
                                        ).await?;
                                        return engine.relay_fallback_no_tls(
                                            TokioSocket::new(raw), upstream,
                                        ).await;
                                    }

                                    // Continue with TLS accept, replay bytes.
                                    // Pass SNI to the protocol handler for routing.
                                    let sni = hello.sni;
                                    let prefixed = PrefixedSocket::from_prefix(
                                        TokioSocket::new(raw), hello.consumed,
                                    );
                                    match acceptor.accept(prefixed).await {
                                        Ok(tls_stream) => engine.handle_vless_stream(
                                            InboundTlsStream::new_generic(tls_stream),
                                            inbound_tag.as_str(), &vless_users,
                                            ws_config.as_ref(), grpc_config.as_ref(),
                                            h2_config.as_ref(),
                                            split_http_config.as_ref(), split_http_registry.as_ref(), http_upgrade_config.as_ref(), fallback_config.as_ref(),
                                            sni,
                                        ).await,
                                        Err(error) => Err(error.into()),
                                    }
                                } else {
                                    // Not valid TLS — direct TLS accept without peek
                                    match acceptor.accept(raw).await {
                                        Ok(tls_stream) => engine.handle_vless_stream(
                                            InboundTlsStream::new(tls_stream),
                                            inbound_tag.as_str(), &vless_users,
                                            ws_config.as_ref(), grpc_config.as_ref(),
                                            h2_config.as_ref(),
                                            split_http_config.as_ref(), split_http_registry.as_ref(), http_upgrade_config.as_ref(), fallback_config.as_ref(),
                                            None,
                                        ).await,
                                        Err(error) => Err(error.into()),
                                    }
                                }
                            }
                            (None, Some(reality)) => {
                                match upgrade_vless_reality_server(stream, &reality).await {
                                    Ok(reality_stream) => {
                                        engine
                                            .handle_vless_stream(
                                                reality_stream,
                                                inbound_tag.as_str(),
                                                &vless_users,
                                                ws_config.as_ref(),
                                                grpc_config.as_ref(),
                                            h2_config.as_ref(),
                                            split_http_config.as_ref(),
                                            split_http_registry.as_ref(),
                                            http_upgrade_config.as_ref(),
                                            fallback_config.as_ref(),
                                            None,
                                            )
                                            .await
                                    }
                                    Err(error) => Err(error.into()),
                                }
                            }
                            (None, None) => {
                                engine
                                    .handle_vless_stream(
                                        stream,
                                        inbound_tag.as_str(),
                                        &vless_users,
                                        ws_config.as_ref(),
                                        grpc_config.as_ref(),
                                        h2_config.as_ref(),
                                        split_http_config.as_ref(),
                                        split_http_registry.as_ref(),
                                        http_upgrade_config.as_ref(),
                                        fallback_config.as_ref(),
                                        None,
                                    )
                                    .await
                            }
                            (Some(_), Some(_)) => Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "vless inbound cannot set both tls and reality",
                            )
                            .into()),
                        };

                        if let Err(ref error) = result {
                            log_listener_connection_error(
                                "vless",
                                inbound_tag.as_str(),
                                &remote_addr,
                                error,
                            );
                        }
                        result
                    });
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "vless connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "vless connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            listen = %local_addr,
            "inbound listener stopped"
        );

        Ok(())
    }

    async fn run_vless_quic_accept_loop(
        &self,
        inbound: &zero_config::InboundConfig,
        quic_inbound: &crate::transport::QuicInbound,
        shutdown: &mut watch::Receiver<bool>,
        connections: &mut JoinSet<Result<(), EngineError>>,
        fallback_config: Option<zero_config::FallbackConfig>,
    ) -> Result<(), EngineError> {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = quic_inbound.accept() => {
                    match accept_result {
                        Ok(quic_stream) => {
                            let engine = self.clone();
                            let inbound_tag = inbound.tag.clone();
                            let vless_users: Arc<[zero_config::VlessUserConfig]> =
                                inbound.protocol.vless_users().into();
                            let fallback_config = fallback_config.clone();

                            connections.spawn(async move {
                                let result = engine
                                    .handle_vless_client(
                                        quic_stream,
                                        inbound_tag.as_str(),
                                        &vless_users, fallback_config.as_ref(),
                                        None,
                                    )
                                    .await;

                                if let Err(error) = &result {
                                    log_listener_connection_error(
                                        "vless",
                                        inbound_tag.as_str(),
                                        &"quic".parse().unwrap_or(std::net::SocketAddr::from(([0, 0, 0, 0], 0))),
                                        error,
                                    );
                                }
                                result
                            });
                        }
                        Err(error) => {
                            error!(error = %error, "vless quic accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "vless quic connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "vless quic connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            transport = "quic",
            "inbound listener stopped"
        );

        Ok(())
    }

    async fn handle_vless_stream<S>(
        &self,
        stream: S,
        inbound_tag: &str,
        users: &[VlessUserConfig],
        ws_config: Option<&zero_config::WebSocketConfig>,
        grpc_config: Option<&zero_config::GrpcConfig>,
        h2_config: Option<&zero_config::H2Config>,
        split_http_config: Option<&zero_config::SplitHttpConfig>,
        split_http_registry: Option<&crate::transport::SplitHttpRegistry>,
        http_upgrade_config: Option<&zero_config::HttpUpgradeConfig>,
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        if let (Some(cfg), Some(reg)) = (split_http_config, split_http_registry) {
            match crate::transport::accept_split_http(stream, cfg, reg).await? {
                Some(split_stream) => {
                    return self
                        .handle_vless_client(split_stream, inbound_tag, users, fallback, sni)
                        .await;
                }
                None => return Ok(()), // consumed by partner connection
            }
        }
        if let Some(cfg) = http_upgrade_config {
            let upg_stream = crate::transport::accept_http_upgrade(stream, cfg).await?;
            return self
                .handle_vless_client(upg_stream, inbound_tag, users, fallback, sni)
                .await;
        }
        match (ws_config, grpc_config, h2_config) {
            (Some(ws), None, None) => {
                let ws_stream = accept_ws(stream, &ws.path).await?;
                self.handle_vless_client(ws_stream, inbound_tag, users, fallback, sni)
                    .await
            }
            (None, Some(grpc), None) => {
                let engine = self.clone();
                let tag = inbound_tag.to_owned();
                let service_names = grpc.service_names.clone();
                let users_arc: Arc<[VlessUserConfig]> = users.into();
                let fb_clone = fallback.cloned();
                return crate::transport::serve_grpc(stream, &service_names, move |grpc_stream| {
                    let engine = engine.clone();
                    let tag = tag.clone();
                    let users = Arc::clone(&users_arc);
                    let fb = fb_clone.clone();
                    async move {
                        engine
                            .handle_vless_client(grpc_stream, &tag, &users, fb.as_ref(), None)
                            .await
                    }
                })
                .await;
            }
            (None, None, Some(h2)) => {
                let h2_stream = crate::transport::accept_h2(stream, h2).await?;
                self.handle_vless_client(h2_stream, inbound_tag, users, fallback, sni)
                    .await
            }
            (None, None, None) => {
                self.handle_vless_client(stream, inbound_tag, users, fallback, sni)
                    .await
            }
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound: ws, grpc, and h2 are mutually exclusive",
            ))),
        }
    }

    pub(crate) async fn handle_vless_client<S>(
        &self,
        client: S,
        inbound_tag: &str,
        users: &[VlessUserConfig],
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let mut metered = MeteredStream::new(RecordingStream::new(client));
        let auth = ConfiguredVlessUsers { users };
        let result = self
            .protocols
            .vless_inbound
            .accept_tcp_with_auth_and_id(&mut metered, &auth)
            .await;

        let (mut session, uuid) = match result {
            Ok(x) => x,
            Err(auth_error) => {
                if let Some(fb) = fallback {
                    let (inner, head) = metered.into_inner().into_parts();
                    return self.relay_fallback(inner, head, fb).await;
                }
                return Err(EngineError::Core(auth_error));
            }
        };

        let (inner_stream, _head) = metered.into_inner().into_parts();
        let client = MeteredStream::new(inner_stream);

        session.sni = sni;

        let auth = session.auth.clone();

        if vless::VlessInbound::is_mux_session(&session) {
            self.handle_vless_mux_session(client, inbound_tag, uuid, &auth)
                .await
        } else if session.network == zero_core::Network::Udp {
            self.handle_vless_udp_session(client, inbound_tag, session, &auth)
                .await
        } else {
            let handler = VlessInboundHandler {
                vless_inbound: self.protocols.vless_inbound,
            };
            let source_addr = client.peer_addr().ok();
            serve_inbound(
                self,
                session,
                TcpRelayStream::new(client.into_inner()),
                &handler,
                inbound_tag,
                source_addr,
            )
            .await
        }
    }

    async fn handle_vless_mux_session<S>(
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
            encode_new_stream_response, parse_new_stream_payload, MuxServer, MUX_STATUS_FAIL,
            MUX_STATUS_OK, MUX_STREAM_NEW,
        };

        self.protocols
            .vless_inbound
            .send_response(&mut client)
            .await?;
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
                    if frame.stream_id == MUX_STREAM_NEW {
                        match parse_new_stream_payload(&frame.payload) {
                            Ok((port, target)) => {
                                let sid = next_id;
                                next_id = next_id.wrapping_add(1);
                                if next_id == 0 { next_id = 1; }

                                // Route and establish outbound
                                let mut session = zero_core::Session::new(
                                    0, target, port, zero_core::Network::Tcp,
                                    zero_core::ProtocolType::Vless,
                                );
                                if let Some(ref a) = auth {
                                    session.apply_auth(a.clone());
                                }
                                self.prepare_session(&mut session, inbound_tag, None);
                                self.resolve_fake_ip_target(&mut session).await;
                let action = self.route_decision(&session);
                                let Ok(resolved) = self.resolve_outbound(&action) else {
                                    let resp = encode_new_stream_response(0, MUX_STATUS_FAIL);
                                    let _ = mux.write_data(&mut client, MUX_STREAM_NEW, &resp).await;
                                    continue;
                                };
                                let upstream = match self.establish_tcp_outbound(&session, resolved).await {
                                    Ok(outbound) => match outbound {
                                        EstablishedTcpOutbound::Direct { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Vless { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Socks5 { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Hysteria2 { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Shadowsocks { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Trojan { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Vmess { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Mieru { upstream, .. } => upstream,
                                        EstablishedTcpOutbound::Relay { upstream } => upstream,
                                        EstablishedTcpOutbound::Block { .. } => {
                                            let resp = encode_new_stream_response(0, MUX_STATUS_FAIL);
                                            let _ = mux.write_data(&mut client, MUX_STREAM_NEW, &resp).await;
                                            continue;
                                        }
                                    },
                                    Err(_) => {
                                        let resp = encode_new_stream_response(0, MUX_STATUS_FAIL);
                                        let _ = mux.write_data(&mut client, MUX_STREAM_NEW, &resp).await;
                                        continue;
                                    }
                                };

                                let resp = encode_new_stream_response(sid, MUX_STATUS_OK);
                                mux.write_data(&mut client, MUX_STREAM_NEW, &resp).await?;

                                let (up_tx, up_rx) = mpsc::unbounded_channel();
                                up_senders.insert(sid, up_tx);
                                let down = down_tx.clone();

                                relay_tasks.spawn(async move {
                                    Self::mux_stream_relay(sid, up_rx, down, upstream).await;
                                });

                                info!(inbound_tag, mux_stream_id = sid, port, "MUX stream accepted");
                            }
                            Err(e) => {
                                warn!(error = %e, "MUX new stream parse failed");
                                let resp = encode_new_stream_response(0, MUX_STATUS_FAIL);
                                let _ = mux.write_data(&mut client, MUX_STREAM_NEW, &resp).await;
                            }
                        }
                    } else if frame.payload.is_empty() {
                        // Client closed this stream
                        up_senders.remove(&frame.stream_id);
                        // Notify client of stream close
                        let _ = mux.write_data(&mut client, frame.stream_id, &[]).await;
                    } else if let Some(tx) = up_senders.get(&frame.stream_id) {
                        let _ = tx.send(frame.payload);
                    }
                }

                down = down_rx.recv() => {
                    if let Some((sid, payload)) = down {
                        if up_senders.contains_key(&sid) {
                            if payload.is_empty() {
                                // Upstream closed — notify client and clean up
                                let _ = mux.write_data(&mut client, sid, &[]).await;
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

    async fn mux_stream_relay(
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

    async fn handle_vless_udp_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        session: zero_core::Session,
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        self.protocols
            .vless_inbound
            .send_response(&mut client)
            .await?;
        self.record_session_inbound_traffic(session.id, client.drain_traffic());

        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless-udp",
            "vless udp session started"
        );

        let mut buffer = vec![0_u8; 64 * 1024];
        let mut udp_buffer = vec![0_u8; 64 * 1024];
        let mut upstream_buffer = vec![0_u8; 64 * 1024];
        let proxy = self.clone();

        loop {
            // Split dispatch into disjoint borrows so select! can pin
            // all futures simultaneously without borrow conflicts.
            // VLESS chain responses now flow through chain_tasks.JoinSet
            // alongside SS/H2/Trojan/Mieru — no separate vless_mgr poll.
            let (direct_sock, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "vless-udp",
                        "vless udp session idle timeout"
                    );
                    break;
                }
                read = client.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            last_activity = TokioInstant::now();
                            self.record_session_inbound_traffic(0, client.drain_traffic());

                            if let Err(error) = Self::vless_dispatch_packet(
                                &proxy,
                                &mut dispatch,
                                &buffer[..n],
                                auth,
                            ).await {
                                warn!(
                                    error = %error,
                                    "failed to process vless udp packet"
                                );
                            }

                        }
                        Err(error) => {
                            warn!(
                                error = %error,
                                "vless udp client read error"
                            );
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut udp_buffer) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                    let target = match ip {
                        zero_traits::IpAddress::V4(bytes) => zero_core::Address::Ipv4(bytes),
                        zero_traits::IpAddress::V6(bytes) => zero_core::Address::Ipv6(bytes),
                    };
                    let port = sender.port();

                    if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                        self.record_session_outbound_rx(session_id, n as u64);
                    }

                    match build_udp_packet(&target, port, &udp_buffer[..n]) {
                        Ok(packet) => {
                            match client.write_all(&packet).await {
                                Ok(_) => {
                                    if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                                        self.record_session_inbound_tx(session_id, packet.len() as u64);
                                    }
                                    self.record_session_inbound_traffic(0, client.drain_traffic());
                                }
                                Err(error) => {
                                    warn!(
                                        error = %error,
                                        "failed to write vless udp response"
                                    );
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(
                                error = %error,
                                "failed to build vless udp response packet"
                            );
                        }
                    }
                }
                upstream = recv_upstream_packet(socks5_up, &mut upstream_buffer) => {
                    // SOCKS5 chain upstream response — re-encode as VLESS.
                    use socks5::parse_udp_packet;
                    match upstream {
                        Ok(read) => {
                            last_activity = TokioInstant::now();
                            if let Ok(pkt) = parse_udp_packet(&upstream_buffer[..read]) {
                                if let Ok(packet) = build_udp_packet(&pkt.target, pkt.port, &pkt.payload) {
                                    let _ = client.write_all(&packet).await;
                                    proxy.record_session_inbound_traffic(0, client.drain_traffic());
                                }
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "SOCKS5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {
                    // SOCKS5 upstream idle timeout — association will be
                    // closed by finish_all() on session end.
                }
                Some(chain_result) = chain_tasks.join_next() => {
                    // Chain-outbound response (SS, H2, VLESS via JoinSet).
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            if let Some(sid) = session_id {
                                proxy.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            if let Ok(packet) = build_udp_packet(&target, port, &payload) {
                                if let Err(error) = client.write_all(&packet).await {
                                    warn!(error = %error, "failed to write chain response");
                                    break;
                                }
                                if let Some(sid) = session_id {
                                    proxy.record_session_inbound_tx(sid, packet.len() as u64);
                                }
                                proxy.record_session_inbound_traffic(0, client.drain_traffic());
                            }
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "chain upstream read error");
                        }
                        Err(join_err) => {
                            warn!(error = %join_err, "chain response task panicked");
                        }
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless-udp",
            "vless udp session ended"
        );

        Ok(())
    }

    /// Parse a VLESS UDP packet and dispatch via the generic `UdpDispatch`.
    async fn vless_dispatch_packet(
        proxy: &Proxy,
        dispatch: &mut UdpDispatch,
        packet: &[u8],
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError> {
        use vless::parse_udp_packet;

        let udp_packet = parse_udp_packet(packet)?;

        dispatch
            .dispatch(
                proxy,
                udp_packet.target,
                udp_packet.port,
                &udp_packet.payload,
                zero_core::ProtocolType::Vless,
                auth.as_ref(),
            )
            .await
            .map(|_| ())
    }
    /// Relay a raw TCP stream (post-ClientHello) to a fallback target.
    /// The ClientHello bytes were already written by the caller.
    async fn relay_fallback_no_tls(
        &self,
        client: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
        upstream: TokioSocket,
    ) -> Result<(), EngineError> {
        let metered_client = MeteredStream::new(client);
        let metered_upstream = MeteredStream::new(upstream);
        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;
        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }

    /// Relay to fallback: replay captured VLESS header bytes, then relay.
    async fn relay_fallback<S>(
        &self,
        client_stream: S,
        head: Vec<u8>,
        fallback: &zero_config::FallbackConfig,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut upstream = self
            .protocols
            .direct_outbound
            .connect_host(&fallback.server, fallback.port, self.resolver.as_ref())
            .await?;

        if !head.is_empty() {
            tokio::io::AsyncWriteExt::write_all(&mut upstream, &head).await?;
        }

        let metered_client = MeteredStream::new(client_stream);
        let metered_upstream = MeteredStream::new(upstream);

        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;

        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }
}

// ── Fallback helpers ──

/// Wraps an inner stream and records all bytes read, for replay to a
/// fallback target when VLESS authentication fails.
struct RecordingStream<S> {
    inner: S,
    recorded: Vec<u8>,
}

impl<S> RecordingStream<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            recorded: Vec::with_capacity(128),
        }
    }
    fn into_parts(self) -> (S, Vec<u8>) {
        (self.inner, self.recorded)
    }
}

impl<S> AsyncRead for RecordingStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let prev = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &result {
            let n = buf.filled().len() - prev;
            if n > 0 {
                self.recorded.extend_from_slice(&buf.filled()[prev..]);
            }
        }
        result
    }
}

impl<S> AsyncWrite for RecordingStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> AsyncSocket for RecordingStream<S>
where
    S: AsyncSocket<Error = io::Error> + Send + Sync,
{
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = self.inner.read(buf).await?;
        self.recorded.extend_from_slice(&buf[..n]);
        Ok(n)
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.inner.shutdown().await
    }
}

impl<S> ClientStream for RecordingStream<S>
where
    S: ClientStream + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

async fn upgrade_vless_reality_server<S>(
    stream: S,
    reality: &InboundRealityConfig,
) -> std::io::Result<vless::RealityTlsStream<S>>
where
    S: ClientStream + 'static,
{
    let server_name = reality.server_name.as_deref().unwrap_or("localhost");
    vless::upgrade_reality_server(
        stream,
        RealityServerOptions {
            private_key: &reality.private_key,
            short_ids: &reality.short_ids,
            server_name,
            cipher_suites: &reality.cipher_suites,
        },
    )
    .await
}

struct ConfiguredVlessUsers<'a> {
    users: &'a [VlessUserConfig],
}

impl VlessUserStore for ConfiguredVlessUsers<'_> {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        self.users.iter().find_map(|user| {
            let configured_id = vless::parse_uuid(&user.id).ok()?;
            if &configured_id == id {
                let flow = user.flow.as_deref().and_then(|f| vless::parse_flow(f).ok());
                Some(VlessUser {
                    credential_id: user.credential_id.clone(),
                    principal_key: user.principal_key.clone(),
                    up_bps: user.up_bps,
                    down_bps: user.down_bps,
                    flow,
                })
            } else {
                None
            }
        })
    }
}
