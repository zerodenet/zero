//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

// Listener.

mod listener;
mod mux;
mod mux_udp;
mod transport;
mod udp_session;

pub(crate) use listener::run_vmess_listener_with_bound;
pub(crate) use transport::{handle_vmess_grpc, handle_vmess_raw, handle_vmess_ws};
