use tokio::task::JoinSet;

use crate::runtime::mux_udp::{run_mux_udp_relay, MuxUdpRelayRequest};
use crate::runtime::Proxy;

pub(super) fn spawn_vmess_mux_udp_stream_task(
    proxy: &Proxy,
    tasks: &mut JoinSet<()>,
    mux_session_id: u16,
    up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    responder: vmess::udp::VmessInboundMuxUdpResponder,
    inbound_tag: String,
) {
    let proxy = proxy.clone();
    tasks.spawn(async move {
        run_mux_udp_relay(
            &proxy,
            MuxUdpRelayRequest {
                mux_session_id,
                up_rx,
                responder,
                inbound_tag: &inbound_tag,
                protocol: "vmess_mux_udp",
                auth: None,
            },
        )
        .await;
    });
}
