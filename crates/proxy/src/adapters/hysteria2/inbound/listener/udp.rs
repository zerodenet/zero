use std::sync::Arc;

use zero_engine::EngineError;

use crate::runtime::datagram_udp::{run_datagram_udp_relay, DatagramUdpRelayRequest};
use crate::runtime::Proxy;

impl Proxy {
    pub(super) async fn hysteria2_datagram_loop(
        conn: Arc<quinn::Connection>,
        responder: hysteria2::Hysteria2InboundUdpResponder,
        inbound_tag: String,
        proxy: Proxy,
    ) -> Result<(), EngineError> {
        run_datagram_udp_relay(
            &proxy,
            DatagramUdpRelayRequest {
                source: conn,
                responder,
                inbound_tag: &inbound_tag,
                poll_upstream: true,
                auth: None,
            },
        )
        .await
    }
}
