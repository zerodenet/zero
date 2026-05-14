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
pub(crate) use tls_hello::*;

// Re-export transport implementations from zero-transport.
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::vless_transport::{build_vless_outbound_transport, VlessTransportConnector};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::grpc::{accept_grpc, connect_grpc, GrpcStream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::h2::{accept_h2, connect_h2, H2Stream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::http_upgrade::{accept_http_upgrade, connect_http_upgrade, HttpUpgradeStream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::quic::{connect_quic, QuicInbound, QuicStream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::tls::{build_tls_acceptor, connect_tls_upstream, InboundTlsStream};
#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
pub(crate) use zero_transport::ws::{accept_ws, connect_ws, WebSocketSocket};
#[cfg(any(feature = "inbound-hysteria2", feature = "outbound-hysteria2"))]
pub(crate) use zero_transport::hysteria2_quic::Hysteria2Stream;
