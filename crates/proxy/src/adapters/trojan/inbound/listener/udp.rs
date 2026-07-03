use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::run_protocol_stream_udp_relay;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn run_trojan_udp_relay(
    proxy: &Proxy,
    session: Session,
    relay: trojan::TrojanInboundUdpRelay<TcpRelayStream>,
    inbound_tag: &str,
) -> Result<(), EngineError> {
    run_protocol_stream_udp_relay(proxy, &session, relay, inbound_tag, "trojan_udp", None).await
}
