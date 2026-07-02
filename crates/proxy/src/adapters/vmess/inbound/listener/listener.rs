use std::io;

use tokio::sync::watch;
use tracing::warn;
use zero_engine::EngineError;

use super::model::VmessInboundRequest;
use super::{handle_vmess_grpc, handle_vmess_raw, handle_vmess_ws};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;

pub(crate) async fn run_vmess_listener_with_bound(
    proxy: &Proxy,
    request: VmessInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let VmessInboundRequest {
        inbound,
        profile,
        tls_acceptor,
        ws: ws_config,
        grpc: grpc_config,
    } = request;
    if profile.is_empty() {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vmess requires at least one user",
        )));
    }

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

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: transport,
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let profile = profile.clone();
            let tls_acceptor = tls_acceptor.clone();
            let ws = ws_config.clone();
            let grpc = grpc_config.clone();
            async move {
                let res = if let Some(grpc_cfg) = &grpc {
                    handle_vmess_grpc(
                        &engine,
                        &tls_acceptor,
                        &profile,
                        stream.into(),
                        grpc_cfg,
                        &tag,
                        source_addr,
                    )
                    .await
                } else if let Some(ws_cfg) = &ws {
                    handle_vmess_ws(
                        &engine,
                        &tls_acceptor,
                        &profile,
                        stream.into(),
                        ws_cfg,
                        &tag,
                        source_addr,
                    )
                    .await
                } else {
                    handle_vmess_raw(
                        &engine,
                        &tls_acceptor,
                        &profile,
                        stream.into(),
                        &tag,
                        source_addr,
                    )
                    .await
                };
                if let Err(e) = res {
                    if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                    {
                        warn!(?source_addr, %e, "vmess failed");
                    }
                }
            }
        },
    })
    .await
}
