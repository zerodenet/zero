#[cfg(any(feature = "vless", feature = "vmess"))]
mod mux;
#[cfg(feature = "vless")]
mod recorded;
#[cfg(feature = "trojan")]
mod stream;

#[cfg(feature = "vmess")]
pub(crate) use mux::{
    dispatch_no_client_mux_route_request_with_defaults, NoClientMuxRouteDefaults,
};
#[cfg(feature = "vless")]
pub(crate) use recorded::{
    dispatch_recorded_protocol_mux_stream_request_with_defaults,
    dispatch_recorded_protocol_mux_tcp_request_with_defaults, RecordedProtocolMuxRouteDefaults,
};
#[cfg(feature = "trojan")]
pub(crate) use stream::dispatch_no_client_stream_route;
