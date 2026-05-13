mod direct;
mod metered;
mod stream;
mod tcp_flow;
mod tcp_outbound;
mod tcp_relay;
pub(crate) mod tls_hello;

pub(crate) use direct::*;
pub(crate) use metered::*;
pub(crate) use stream::*;
pub(crate) use tcp_flow::*;
pub(crate) use tcp_outbound::*;
pub(crate) use tcp_relay::*;

// Transport implementations moved to zero_protocol_vless.
// Re-export items still used directly by proxy code.
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_protocol_vless::{
    accept_grpc, accept_h2, accept_http_upgrade, accept_ws, build_tls_acceptor, connect_quic,
    InboundTlsStream, QuicInbound,
};
