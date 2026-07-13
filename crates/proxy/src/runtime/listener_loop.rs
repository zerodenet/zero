#[cfg(any(feature = "hysteria2", feature = "vless"))]
mod quic;
mod system;
mod tcp;
#[cfg(test)]
mod tests;

#[cfg(feature = "vless")]
pub(crate) use quic::{run_logged_quic_stream_listener_loop, LoggedQuicStreamListenerRequest};
#[cfg(feature = "hysteria2")]
pub(crate) use quic::{run_quic_listener_loop, QuicListenerLoopRequest};
pub(crate) use system::{run_system_tcp_stack_loop, SystemTcpStackLoopRequest};
pub(crate) use tcp::{run_logged_tcp_socket_listener_loop, LoggedTcpSocketListenerRequest};
#[cfg(test)]
pub(crate) use tcp::{run_tcp_listener_loop, TcpListenerLoopRequest};
