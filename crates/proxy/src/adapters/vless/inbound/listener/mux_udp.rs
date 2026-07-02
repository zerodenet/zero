use crate::runtime::mux_udp::{run_mux_udp_relay, MuxUdpRelayRequest};
use crate::runtime::Proxy;

pub(super) async fn spawn_vless_mux_udp_stream_task(
    proxy: &Proxy,
    mux_session_id: u16,
    up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    responder: vless::udp::VlessInboundMuxUdpResponder,
    inbound_tag: &str,
    auth: Option<zero_core::SessionAuth>,
) {
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
