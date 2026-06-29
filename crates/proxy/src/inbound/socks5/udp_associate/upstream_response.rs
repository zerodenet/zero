use std::net::SocketAddr;

use tracing::debug;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::logging::log_udp_upstream_association_dropped;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    record_udp_inbound_response_rx, record_udp_inbound_response_tx,
};
use crate::runtime::Proxy;

pub(super) async fn handle_upstream_response(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &str,
    upstream: Result<usize, EngineError>,
    buf: &[u8],
) -> Result<(), EngineError> {
    match upstream {
        Ok(read) => {
            proxy.record_udp_upstream_packet_received();
            dispatch.touch_upstream_idle(proxy.udp_upstream_idle_timeout());
            forward_upstream_response(
                proxy,
                dispatch,
                relay,
                client_addr,
                inbound_tag,
                &buf[..read],
            )
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
    payload: &[u8],
) -> Result<(), EngineError> {
    let session_id = upstream_response_session_id(dispatch, inbound_tag, payload);

    let Some(client_addr) = client_addr else {
        return Ok(());
    };

    record_udp_inbound_response_rx(proxy, session_id, payload.len());
    let udp_session = socks5::Socks5Inbound.udp_session();
    let sent = udp_session
        .send_encoded_response_to_client(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))?;
    record_udp_inbound_response_tx(proxy, session_id, sent);

    Ok(())
}

fn upstream_response_session_id(
    dispatch: &UdpDispatch,
    inbound_tag: &str,
    payload: &[u8],
) -> Option<u64> {
    let association = dispatch.upstream_association_view()?;
    let udp_session = socks5::Socks5Inbound.udp_session();
    match udp_session.response_session_key_parts(payload) {
        Ok((target, port)) => {
            dispatch.upstream_response_session_id(association.outbound_tag, &target, port)
        }
        Err(error) => {
            debug!(
                inbound_tag = inbound_tag,
                outbound_tag = association.outbound_tag,
                error = %error,
                "failed to attribute upstream UDP response"
            );
            None
        }
    }
}
