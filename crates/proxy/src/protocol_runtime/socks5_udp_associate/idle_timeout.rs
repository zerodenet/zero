use crate::logging::log_udp_upstream_association_idle_timeout;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;

pub(super) fn handle_idle_timeout(proxy: &Proxy, dispatch: &mut UdpDispatch, inbound_tag: &str) {
    if let Some(closed) = dispatch.drop_idle_upstream_association() {
        log_udp_upstream_association_idle_timeout(
            inbound_tag,
            &closed.outbound_tag,
            &closed.server,
            closed.port,
            proxy.udp_upstream_idle_timeout(),
        );
    }
}
