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

// Re-export transport implementations from zero-transport.
// Only items used directly by proxy code are listed.
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::grpc::serve_grpc;
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::h2::accept_h2;
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::http_upgrade::accept_http_upgrade;
#[cfg(any(feature = "inbound-hysteria2", feature = "outbound-hysteria2"))]
pub(crate) use zero_transport::hysteria2_quic::{Hysteria2Connector, Hysteria2Stream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::quic::{connect_quic, QuicInbound};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::split_http::{accept_split_http, SplitHttpRegistry};
#[cfg(any(
    feature = "inbound-vless",
    feature = "outbound-vless",
    feature = "inbound-trojan",
    feature = "outbound-trojan"
))]
pub(crate) use zero_transport::tls::{build_tls_acceptor, InboundTlsStream, TlsAcceptor};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::vless_transport::VlessTransportConnector;
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::ws::accept_ws;
