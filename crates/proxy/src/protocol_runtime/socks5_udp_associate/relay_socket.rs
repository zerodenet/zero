use std::net::SocketAddr;

use tracing::{debug, warn};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use super::{direct_response, dispatch};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

pub(super) struct RelayPacketRequest<'a> {
    pub proxy: &'a Proxy,
    pub dispatch: &'a mut UdpDispatch,
    pub relay: &'a TokioDatagramSocket,
    pub inbound_tag: &'a str,
    pub pending_control_traffic: &'a mut StreamTraffic,
    pub client_udp_addr: &'a mut Option<SocketAddr>,
    pub sender: SocketAddr,
    pub payload: &'a [u8],
}

pub(super) async fn handle_relay_packet(
    request: RelayPacketRequest<'_>,
) -> Result<(), EngineError> {
    if request.client_udp_addr.is_none() {
        *request.client_udp_addr = Some(request.sender);
    }

    if *request.client_udp_addr == Some(request.sender) {
        if let Err(error) = dispatch::dispatch_packet(
            request.proxy,
            request.payload,
            request.dispatch,
            request.pending_control_traffic,
        )
        .await
        {
            warn!(
                inbound_tag = request.inbound_tag,
                protocol = "socks5_udp",
                error = %error,
                "failed to process UDP packet"
            );
        }

        return Ok(());
    }

    if let Some(client_addr) = *request.client_udp_addr {
        direct_response::forward_relay_socket_response(
            request.proxy,
            request.dispatch,
            request.relay,
            client_addr,
            request.sender,
            request.payload,
        )
        .await?;
    } else {
        debug!(?request.sender, "dropping udp packet from unexpected sender");
    }

    Ok(())
}
