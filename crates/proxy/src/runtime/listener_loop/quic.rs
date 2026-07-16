#[cfg(feature = "hysteria2")]
mod connection;
#[cfg(feature = "vless")]
mod logged;
#[cfg(feature = "vless")]
mod stream;

#[cfg(feature = "hysteria2")]
pub(crate) use connection::{run_quic_listener_loop, QuicListenerLoopRequest};
#[cfg(feature = "vless")]
pub(crate) use logged::{run_logged_quic_stream_listener_loop, LoggedQuicStreamListenerRequest};
