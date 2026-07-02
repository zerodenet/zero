//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use vmess::{VmessInbound, VmessInboundProfile};

// Trait-based handler (raw TLS path).

#[derive(Clone)]
pub(crate) struct VmessInboundHandler {
    vmess_inbound: VmessInbound,
    profile: VmessInboundProfile,
    tls_acceptor: crate::transport::TlsAcceptor,
}

// Listener.

mod listener;
pub(crate) mod model;
mod mux;
mod mux_udp;
mod transport;
mod udp_session;

pub(crate) use listener::run_vmess_listener_with_bound;
pub(crate) use transport::{handle_vmess_grpc, handle_vmess_raw, handle_vmess_ws};
