use crate::runtime::Proxy;

pub(super) struct RecordedProtocolMuxDispatch<P> {
    pub(super) proxy: Proxy,
    pub(super) inbound_tag: String,
    pub(super) source_addr: Option<std::net::SocketAddr>,
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

impl From<zero_transport::inbound_route::RecordedMuxRouteDefaults>
    for RecordedProtocolMuxRouteDefaults
{
    fn from(defaults: zero_transport::inbound_route::RecordedMuxRouteDefaults) -> Self {
        Self {
            udp_protocol: defaults.udp_protocol,
            mux_protocol: defaults.mux_protocol,
            panic_message: defaults.panic_message,
            abort_on_end: defaults.abort_on_end,
            udp_accept_log_message: defaults.udp_accept_log_message,
        }
    }
}
