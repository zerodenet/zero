use zero_config::InboundConfig;
use zero_engine::EngineError;
use zero_transport::trojan_transport::TrojanInboundListenerRequest;

use crate::protocol_registry::BoundInbound;
use crate::runtime::inbound_route::dispatch_no_client_stream_route;
use crate::runtime::listener_loop::{
    run_logged_tcp_socket_listener_loop, LoggedTcpSocketListenerRequest,
};
use crate::runtime::Proxy;

pub(super) fn spawn(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
) {
    let proxy = proxy.clone();
    listeners.spawn(async move {
        let request = TrojanInboundListenerRequest::from_protocol_config(
            &inbound.protocol,
            proxy.config.source_dir(),
        )?;
        let protocol_name = request.protocol_name();
        let error_protocol_name = request.error_protocol_name();

        run_logged_tcp_socket_listener_loop(LoggedTcpSocketListenerRequest {
            proxy: &proxy,
            inbound_tag: inbound.tag,
            protocol_name,
            error_protocol_name,
            request,
            listener: bound.into_tcp(),
            shutdown: shutdown_rx,
            dispatch: |proxy,
                       request: TrojanInboundListenerRequest,
                       inbound_tag,
                       socket,
                       source_addr| async move {
                let defaults = request.no_client_stream_route_defaults();
                let route = request.accept_route(socket).await?;
                dispatch_no_client_stream_route(
                    route,
                    proxy,
                    inbound_tag,
                    source_addr,
                    defaults.udp_protocol,
                )
                .await
            },
        })
        .await
    });
}
