#[cfg(any(feature = "hysteria2", feature = "transport_quic"))]
mod quic;
mod system;
mod tcp;

#[cfg(feature = "transport_quic")]
#[allow(unused_imports)]
pub(crate) use quic::{
    run_logged_quic_stream_listener_loop, run_quic_stream_listener_loop,
    LoggedQuicStreamListenerRequest, QuicStreamListenerLoopRequest,
};
#[cfg(feature = "hysteria2")]
pub(crate) use quic::{run_quic_listener_loop, QuicListenerLoopRequest};
pub(crate) use system::{run_system_tcp_stack_loop, SystemTcpStackLoopRequest};
#[allow(unused_imports)]
pub(crate) use tcp::{
    run_logged_tcp_socket_listener_loop, run_tcp_listener_loop, LoggedTcpSocketListenerRequest,
    TcpListenerLoopRequest,
};
