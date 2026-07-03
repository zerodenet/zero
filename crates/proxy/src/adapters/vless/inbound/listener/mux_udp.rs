use crate::runtime::mux_udp::run_protocol_mux_udp_relay;
use crate::runtime::Proxy;

pub(super) async fn spawn_vless_mux_udp_stream_task(
    proxy: &Proxy,
    relay: vless::mux::VlessInboundMuxUdpRelay,
    inbound_tag: &str,
) {
    run_protocol_mux_udp_relay(proxy, relay, inbound_tag, "vless_mux_udp").await;
}
