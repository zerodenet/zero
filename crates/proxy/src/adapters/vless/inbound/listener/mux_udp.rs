use crate::runtime::mux_udp::{run_mux_udp_relay, MuxUdpRelayRequest};
use crate::runtime::Proxy;

pub(super) async fn spawn_vless_mux_udp_stream_task(
    proxy: &Proxy,
    relay: vless::mux::VlessInboundMuxUdpRelay,
    inbound_tag: &str,
) {
    let (mux_session_id, _port, up_rx, responder, auth) = relay.into_parts();
    run_mux_udp_relay(
        proxy,
        MuxUdpRelayRequest {
            mux_session_id,
            up_rx,
            responder,
            inbound_tag,
            protocol: "vless_mux_udp",
            auth,
        },
    )
    .await;
}
