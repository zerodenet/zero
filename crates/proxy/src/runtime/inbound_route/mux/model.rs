use crate::runtime::route_runtime::InboundRouteRuntime;

pub(crate) struct MuxRouteBridge<P, FMapTcp, FRunUdp, FRunMux> {
    pub(crate) runtime: InboundRouteRuntime,
    pub(crate) protocol: P,
    pub(crate) map_tcp_stream: FMapTcp,
    pub(crate) run_udp: FRunUdp,
    pub(crate) run_mux: FRunMux,
}

#[cfg(feature = "managed-stream-runtime")]
#[derive(Clone, Copy)]
pub(crate) struct NoClientMuxRouteDefaults {
    pub(crate) udp_protocol: &'static str,
    pub(crate) mux_protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
    pub(crate) read_error_log: &'static str,
}
