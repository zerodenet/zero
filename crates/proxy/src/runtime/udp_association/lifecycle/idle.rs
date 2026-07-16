use crate::logging::log_udp_upstream_association_idle_timeout;
use crate::runtime::udp_dispatch::UdpDispatch;

use super::relay::UdpAssociationLoopContext;

pub(super) fn handle_idle_timeout(
    context: &UdpAssociationLoopContext<'_>,
    dispatch: &mut UdpDispatch,
) {
    if let Some(closed) = dispatch.drop_idle_upstream_association() {
        log_udp_upstream_association_idle_timeout(
            context.inbound_tag,
            &closed.outbound_tag,
            &closed.server,
            closed.port,
            context.runtime.services().udp_upstream_idle_timeout(),
        );
    }
}
