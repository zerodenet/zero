use std::net::SocketAddr;

use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::logging::log_udp_upstream_association_dropped;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::record_upstream_udp_response_received;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::Proxy;

pub(super) async fn handle_upstream_response(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &str,
    upstream: Result<UpstreamUdpResponse, EngineError>,
) -> Result<(), EngineError> {
    match upstream {
        Ok(response) => {
            forward_upstream_response(proxy, dispatch, relay, client_addr, response).await
        }
        Err(error) => {
            if let Some(closed) = dispatch.drop_upstream_association() {
                proxy.record_udp_upstream_recv_failure();
                log_udp_upstream_association_dropped(
                    inbound_tag,
                    &closed.outbound_tag,
                    &closed.server,
                    closed.port,
                    &error,
                );
            }
            Ok(())
        }
    }
}

async fn forward_upstream_response(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    response: UpstreamUdpResponse,
) -> Result<(), EngineError> {
    let response = record_upstream_udp_response_received(
        proxy,
        dispatch,
        proxy.udp_upstream_idle_timeout(),
        response,
    );

    let Some(client_addr) = client_addr else {
        return Ok(());
    };

    let udp_session = socks5::Socks5Inbound.udp_session();
    let sent = udp_session
        .send_response_to_client_target(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            &response.target,
            response.port,
            &response.payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))?;
    response.accounting.record_sent(sent);

    Ok(())
}
