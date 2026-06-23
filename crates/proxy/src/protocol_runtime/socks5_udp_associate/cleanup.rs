use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::log_completed_udp_flow;

pub(super) fn finish_dispatch(dispatch: UdpDispatch) {
    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }
}
