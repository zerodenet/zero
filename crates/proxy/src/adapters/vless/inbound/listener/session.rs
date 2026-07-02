use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::Proxy;
use crate::transport::{accept_ws, ClientStream, MeteredStream, RecordingStream, TcpRelayStream};
use zero_core::{Session, SessionAuth};
use zero_engine::EngineError;

use super::VlessInboundHandler;

#[derive(Clone, Copy)]
pub(crate) struct VlessStreamTransport<'a> {
    pub(crate) ws_config: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc_config: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2_config: Option<&'a zero_config::H2Config>,
    pub(crate) split_http_config: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) split_http_registry: Option<&'a crate::transport::SplitHttpRegistry>,
    pub(crate) http_upgrade_config: Option<&'a zero_config::HttpUpgradeConfig>,
}

pub(crate) struct VlessStreamRequest<'a, S> {
    pub(crate) stream: S,
    pub(crate) inbound_tag: &'a str,
    pub(crate) profile: vless::VlessInboundProfile,
    pub(crate) transport: VlessStreamTransport<'a>,
    pub(crate) fallback: Option<&'a zero_config::FallbackConfig>,
    pub(crate) sni: Option<String>,
}

struct VlessAcceptedClientBridge<'a> {
    proxy: &'a Proxy,
    handler: &'a VlessInboundHandler,
    inbound_tag: &'a str,
}

impl<S> vless::VlessAcceptedClientRouteDispatcher<MeteredStream<RecordingStream<S>>>
    for VlessAcceptedClientBridge<'_>
where
    S: ClientStream + 'static,
{
    type Error = EngineError;

    async fn dispatch_tcp_session(
        &mut self,
        session: Session,
        metered: MeteredStream<RecordingStream<S>>,
    ) -> Result<(), Self::Error> {
        let client = metered.into_unrecorded_inner();
        let source_addr = client.peer_addr().ok();
        serve_inbound(
            self.proxy,
            session,
            TcpRelayStream::new(client),
            self.handler,
            self.inbound_tag,
            source_addr,
        )
        .await
    }

    async fn dispatch_udp_session(
        &mut self,
        session: Session,
        auth: Option<SessionAuth>,
        responder: vless::VlessInboundUdpResponder,
        mut metered: MeteredStream<RecordingStream<S>>,
    ) -> Result<(), Self::Error> {
        self.proxy
            .record_session_inbound_traffic(session.id, metered.drain_traffic());
        let client = MeteredStream::new(metered.into_unrecorded_inner());
        self.proxy
            .handle_vless_udp_session(client, self.inbound_tag, session, responder, auth)
            .await
    }

    async fn dispatch_mux_session(
        &mut self,
        mux_server: vless::mux::VlessInboundMuxServer,
        mut metered: MeteredStream<RecordingStream<S>>,
    ) -> Result<(), Self::Error> {
        self.proxy
            .record_session_inbound_traffic(0, metered.drain_traffic());
        let client = MeteredStream::new(metered.into_unrecorded_inner());
        self.proxy
            .handle_vless_mux_session(client, self.inbound_tag, mux_server)
            .await
    }
}

impl Proxy {
    pub(crate) async fn handle_vless_stream<S>(
        &self,
        request: VlessStreamRequest<'_, S>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let VlessStreamRequest {
            stream,
            inbound_tag,
            profile,
            transport,
            fallback,
            sni,
        } = request;
        let VlessStreamTransport {
            ws_config,
            grpc_config,
            h2_config,
            split_http_config,
            split_http_registry,
            http_upgrade_config,
        } = transport;

        if let Some(cfg) = split_http_config {
            let Some(reg) = split_http_registry else {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "vless inbound: split-http registry is required",
                )));
            };
            match crate::transport::accept_xhttp_inbound(stream, cfg, reg).await? {
                Some(xhttp_stream) => {
                    return self
                        .handle_vless_client(xhttp_stream, inbound_tag, profile, fallback, sni)
                        .await;
                }
                None => return Ok(()),
            }
        }
        if let Some(cfg) = http_upgrade_config {
            let upg_stream = crate::transport::accept_http_upgrade(stream, cfg).await?;
            return self
                .handle_vless_client(upg_stream, inbound_tag, profile, fallback, sni)
                .await;
        }
        match (ws_config, grpc_config, h2_config) {
            (Some(ws), None, None) => {
                let ws_stream = accept_ws(stream, &ws.path).await?;
                self.handle_vless_client(ws_stream, inbound_tag, profile, fallback, sni)
                    .await
            }
            (None, Some(grpc), None) => {
                let engine = self.clone();
                let tag = inbound_tag.to_owned();
                let service_names = grpc.service_names.clone();
                let profile = profile.clone();
                let fb_clone = fallback.cloned();
                return crate::transport::serve_grpc(stream, &service_names, move |grpc_stream| {
                    let engine = engine.clone();
                    let tag = tag.clone();
                    let profile = profile.clone();
                    let fb = fb_clone.clone();
                    async move {
                        engine
                            .handle_vless_client(grpc_stream, &tag, profile, fb.as_ref(), None)
                            .await
                    }
                })
                .await;
            }
            (None, None, Some(h2)) => {
                let h2_stream = crate::transport::accept_h2(stream, h2).await?;
                self.handle_vless_client(h2_stream, inbound_tag, profile, fallback, sni)
                    .await
            }
            (None, None, None) => {
                self.handle_vless_client(stream, inbound_tag, profile, fallback, sni)
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
        profile: vless::VlessInboundProfile,
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let metered = MeteredStream::new(RecordingStream::new(client));
        let route = match profile.accept_client(vless::VlessInbound, metered).await {
            Ok(accepted) => accepted.into_route_with_sni(sni),
            Err(rejected) => {
                let (auth_error, fallback_replay) = rejected.into_fallback_replay();
                if let Some(fb) = fallback {
                    return self.relay_fallback(fallback_replay, fb).await;
                }
                return Err(EngineError::Core(auth_error));
            }
        };

        let handler = VlessInboundHandler {
            vless_inbound: vless::VlessInbound,
        };
        let mut bridge = VlessAcceptedClientBridge {
            proxy: self,
            handler: &handler,
            inbound_tag,
        };
        route.dispatch_with(&mut bridge).await
    }
}
