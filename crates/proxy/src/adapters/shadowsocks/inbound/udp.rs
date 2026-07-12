//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::sync::Arc;

use tokio::net::UdpSocket;
use zero_core::InboundDatagramUdpRelay;
use zero_engine::EngineError;

use crate::runtime::datagram_udp::run_protocol_datagram_udp_relay;
use crate::runtime::Proxy;

pub(super) async fn ss_udp_relay_loop<R>(
    proxy: &Proxy,
    udp_socket: Arc<UdpSocket>,
    inbound_tag: &str,
    relay: R,
) -> Result<(), EngineError>
where
    R: InboundDatagramUdpRelay<Arc<UdpSocket>>,
{
    run_protocol_datagram_udp_relay(proxy, udp_socket, relay, inbound_tag, false).await
}
