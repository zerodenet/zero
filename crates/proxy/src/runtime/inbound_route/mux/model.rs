use crate::runtime::Proxy;

pub(crate) struct MuxRouteBridge<P, FMapTcp, FRunUdp, FRunMux> {
    pub(crate) proxy: Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) source_addr: Option<std::net::SocketAddr>,
    pub(crate) protocol: P,
    pub(crate) map_tcp_stream: FMapTcp,
    pub(crate) run_udp: FRunUdp,
    pub(crate) run_mux: FRunMux,
}

#[cfg(feature = "vmess")]
#[derive(Clone, Copy)]
pub(crate) struct NoClientMuxRouteDefaults {
    pub(crate) udp_protocol: &'static str,
    pub(crate) mux_protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
    pub(crate) read_error_log: &'static str,
}

#[cfg(feature = "vmess")]
impl From<zero_transport::inbound_route::NoClientMuxRouteDefaults> for NoClientMuxRouteDefaults {
    fn from(defaults: zero_transport::inbound_route::NoClientMuxRouteDefaults) -> Self {
        Self {
            udp_protocol: defaults.udp_protocol,
            mux_protocol: defaults.mux_protocol,
            panic_message: defaults.panic_message,
            abort_on_end: defaults.abort_on_end,
            read_error_log: defaults.read_error_log,
        }
    }
}
