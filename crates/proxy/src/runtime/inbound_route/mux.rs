mod dispatch;
mod model;
#[cfg(feature = "vmess")]
mod no_client;
#[cfg(all(test, feature = "vless"))]
mod tests;

#[cfg(feature = "vless")]
pub(super) use dispatch::dispatch_protocol_mux_route;
#[cfg(feature = "vless")]
pub(super) use model::MuxRouteBridge;
#[cfg(feature = "vmess")]
pub(crate) use model::NoClientMuxRouteDefaults;
#[cfg(feature = "vmess")]
pub(crate) use no_client::dispatch_no_client_mux_route_request_with_defaults;
