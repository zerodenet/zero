mod dispatch;
mod model;
#[cfg(feature = "managed-stream-runtime")]
mod no_client;
#[cfg(all(test, feature = "managed-stream-runtime"))]
mod tests;

#[cfg(feature = "managed-stream-runtime")]
pub(super) use dispatch::dispatch_protocol_mux_route;
#[cfg(feature = "managed-stream-runtime")]
pub(super) use model::MuxRouteBridge;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use model::NoClientMuxRouteDefaults;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use no_client::dispatch_no_client_mux_route_request_with_defaults;
