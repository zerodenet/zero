use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::Proxy;
use crate::transport::{accept_ws, ClientStream, MeteredStream, RecordingStream, TcpRelayStream};
use zero_core::{Session, SessionAuth};
use zero_engine::EngineError;

use super::fallback::relay_fallback;
use super::mux::handle_vless_mux_session;
use super::udp_session::handle_vless_udp_session;

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_vless_stream<S>(
    proxy: &Proxy,
    stream: S,
    inbound_tag: &str,
    profile: vless::VlessInboundProfile,
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
    if let Some(cfg) = split_http_config {
        let Some(reg) = split_http_registry else {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound: split-http registry is required",
            )));
        };
        match crate::transport::accept_xhttp_inbound(stream, cfg, reg).await? {
            Some(xhttp_stream) => {
                return handle_vless_client(
                    proxy,
                    xhttp_stream,
                    inbound_tag,
                    profile,
                    fallback,
                    sni,
                )
                .await;
            }
            None => return Ok(()),
        }
    }
    if let Some(cfg) = http_upgrade_config {
        let upg_stream = crate::transport::accept_http_upgrade(stream, cfg).await?;
        return handle_vless_client(proxy, upg_stream, inbound_tag, profile, fallback, sni).await;
    }
    match (ws_config, grpc_config, h2_config) {
        (Some(ws), None, None) => {
            let ws_stream = accept_ws(stream, &ws.path).await?;
            handle_vless_client(proxy, ws_stream, inbound_tag, profile, fallback, sni).await
        }
        (None, Some(grpc), None) => {
            let engine = proxy.clone();
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
                    handle_vless_client(&engine, grpc_stream, &tag, profile, fb.as_ref(), None)
                        .await
                }
            })
            .await;
        }
        (None, None, Some(h2)) => {
            let h2_stream = crate::transport::accept_h2(stream, h2).await?;
            handle_vless_client(proxy, h2_stream, inbound_tag, profile, fallback, sni).await
        }
        (None, None, None) => {
            handle_vless_client(proxy, stream, inbound_tag, profile, fallback, sni).await
        }
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "vless inbound: ws, grpc, and h2 are mutually exclusive",
        ))),
    }
}

pub(super) async fn handle_vless_client<S>(
    proxy: &Proxy,
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
                return relay_fallback(proxy, fallback_replay, fb).await;
            }
            return Err(EngineError::Core(auth_error));
        }
    };

    let protocol = vless::VlessInbound;
    route
        .dispatch(
            |session: Session, metered: MeteredStream<RecordingStream<S>>| async {
                let client = metered.into_unrecorded_inner();
                let source_addr = client.peer_addr().ok();
                serve_inbound(
                    proxy,
                    session,
                    TcpRelayStream::new(client),
                    &protocol,
                    inbound_tag,
                    source_addr,
                )
                .await
            },
            |session: Session,
             auth: Option<SessionAuth>,
             responder: vless::udp::VlessInboundUdpResponder,
             mut metered: MeteredStream<RecordingStream<S>>| async move {
                proxy.record_session_inbound_traffic(session.id, metered.drain_traffic());
                let client = MeteredStream::new(metered.into_unrecorded_inner());
                handle_vless_udp_session(proxy, client, inbound_tag, session, responder, auth).await
            },
            |mux_server: vless::mux::VlessInboundMuxServer,
             mut metered: MeteredStream<RecordingStream<S>>| async move {
                proxy.record_session_inbound_traffic(0, metered.drain_traffic());
                let client = MeteredStream::new(metered.into_unrecorded_inner());
                handle_vless_mux_session(proxy, client, inbound_tag, mux_server).await
            },
        )
        .await
}
