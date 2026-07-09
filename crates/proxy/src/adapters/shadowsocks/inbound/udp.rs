//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::sync::Arc;

use tokio::net::UdpSocket;
use zero_engine::EngineError;

use crate::runtime::datagram_udp::{run_datagram_udp_relay, DatagramUdpRelayRequest};
use crate::runtime::Proxy;

pub(super) async fn ss_udp_relay_loop(
    proxy: &Proxy,
    udp_socket: Arc<UdpSocket>,
    inbound_tag: &str,
    accepted: shadowsocks::udp::ShadowsocksInboundAcceptedUdpSession,
) -> Result<(), EngineError> {
    let (responder, auth) = accepted.into_datagram_relay_parts();

    run_datagram_udp_relay(
        proxy,
        DatagramUdpRelayRequest {
            source: udp_socket,
            responder,
            inbound_tag,
            poll_upstream: false,
            auth: Some(auth),
        },
    )
    .await
}
