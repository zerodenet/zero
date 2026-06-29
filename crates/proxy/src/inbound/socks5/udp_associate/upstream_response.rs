use std::net::SocketAddr;

use tracing::debug;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::logging::log_udp_upstream_association_dropped;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::UdpInboundResponseAccounting;
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
            proxy.record_udp_upstream_packet_received();
            dispatch.touch_upstream_idle(proxy.udp_upstream_idle_timeout());
            forward_upstream_response(proxy, dispatch, relay, client_addr, inbound_tag, response)
                .await
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
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &str,
    response: UpstreamUdpResponse,
) -> Result<(), EngineError> {
    let (target, port, payload) = response.into_parts();
    let session_id = upstream_response_session_id(dispatch, inbound_tag, &target, port);

    let Some(client_addr) = client_addr else {
        return Ok(());
    };

    let response_accounting =
        UdpInboundResponseAccounting::record_received(proxy, session_id, payload.len());
    let udp_session = socks5::Socks5Inbound.udp_session();
    let sent = udp_session
        .send_response_to_client_target(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            &target,
            port,
            &payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))?;
    response_accounting.record_sent(sent);

    Ok(())
}

fn upstream_response_session_id(
    dispatch: &UdpDispatch,
    inbound_tag: &str,
    target: &zero_core::Address,
    port: u16,
) -> Option<u64> {
    let association = dispatch.upstream_association_view()?;
    let session_id = dispatch.upstream_response_session_id(association.outbound_tag, target, port);
    if session_id.is_none() {
        debug!(
            inbound_tag = inbound_tag,
            outbound_tag = association.outbound_tag,
            ?target,
            port,
            "failed to attribute upstream UDP response"
        );
    }
    session_id
}
