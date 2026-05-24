#![allow(clippy::too_many_arguments)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::result_large_err)]

#[cfg(feature = "grpc")]
pub mod grpc;
#[cfg(feature = "h2")]
pub mod h2;
#[cfg(feature = "http-upgrade")]
pub mod http_upgrade;
#[cfg(feature = "quic")]
pub mod hysteria2_quic;
#[cfg(feature = "quic")]
pub mod quic;
#[cfg(feature = "split-http")]
pub mod split_http;
#[cfg(feature = "tls")]
pub mod tls;
#[cfg(feature = "vless")]
pub mod vless_transport;
#[cfg(feature = "ws")]
pub mod ws;
