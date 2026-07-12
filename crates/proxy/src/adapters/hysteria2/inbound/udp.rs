use std::sync::Arc;

use zero_core::InboundDatagramUdpRelay;
use zero_engine::EngineError;

use crate::runtime::datagram_udp::run_protocol_datagram_udp_relay;
use crate::runtime::Proxy;

pub(super) async fn hysteria2_datagram_loop<R>(
    conn: Arc<quinn::Connection>,
    relay: R,
    inbound_tag: String,
    proxy: Proxy,
) -> Result<(), EngineError>
where
    R: InboundDatagramUdpRelay<Arc<quinn::Connection>>,
{
    run_protocol_datagram_udp_relay(&proxy, conn, relay, &inbound_tag, true).await
}
