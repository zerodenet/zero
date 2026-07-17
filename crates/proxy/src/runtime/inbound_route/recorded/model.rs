use crate::runtime::route_runtime::InboundRouteRuntime;

pub(super) struct RecordedProtocolMuxDispatch<P> {
    pub(super) runtime: InboundRouteRuntime,
    pub(super) protocol: P,
    pub(super) defaults: RecordedProtocolMuxRouteDefaults,
}
#[derive(Clone, Copy)]
pub(crate) struct RecordedProtocolMuxRouteDefaults {
    pub(crate) udp_protocol: &'static str,
    pub(crate) mux_protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
    pub(crate) udp_accept_log_message: Option<&'static str>,
}
