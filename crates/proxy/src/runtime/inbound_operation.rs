//! Prepared inbound-listener contracts plus focused listener/context modules.
//!
//! The root stays as a facade so TCP, TCP+datagram, authenticated QUIC, and
//! mixed TCP-or-QUIC execution do not regrow into one large implementation
//! bucket.

mod context;
mod contract;
#[cfg(feature = "hysteria2")]
mod quic;
#[cfg(feature = "shadowsocks")]
mod tcp_and_datagram;
mod tcp_listener;
#[cfg(feature = "vless")]
mod tcp_or_quic;

pub(crate) use context::InboundConnectionContext;
pub(crate) use contract::PreparedInboundListenerOperation;
#[cfg(feature = "hysteria2")]
pub(crate) use quic::AuthenticatedQuicInboundListenerOperation;
#[cfg(feature = "shadowsocks")]
pub(crate) use tcp_and_datagram::TcpAndDatagramInboundListenerOperation;
pub(crate) use tcp_listener::TcpInboundListenerOperation;
#[cfg(feature = "vless")]
pub(crate) use tcp_or_quic::TcpOrQuicInboundListenerOperation;
