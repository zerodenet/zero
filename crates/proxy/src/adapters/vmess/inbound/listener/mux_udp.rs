use tokio::task::JoinSet;

use crate::runtime::mux_udp::run_protocol_mux_udp_relay;
use crate::runtime::Proxy;

pub(super) fn spawn_vmess_mux_udp_stream_task(
    proxy: &Proxy,
    tasks: &mut JoinSet<()>,
    relay: vmess::mux::VmessInboundMuxUdpRelay,
    inbound_tag: String,
) {
    let proxy = proxy.clone();
    tasks.spawn(async move {
        run_protocol_mux_udp_relay(&proxy, relay, &inbound_tag, "vmess_mux_udp").await;
    });
}
