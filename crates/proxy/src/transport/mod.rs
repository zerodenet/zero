mod direct;
mod tcp_flow;
mod tcp_outbound;
mod tcp_relay;

pub(crate) use direct::DirectConnector;
pub(crate) use tcp_flow::is_block_error;
pub(crate) use tcp_outbound::{
    extract_tcp_stream, EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult,
};
pub(crate) use tcp_relay::{relay_bidirectional_metered, relay_bidirectional_metered_throttled};
pub(crate) use zero_transport::{
    ClientStream, MeteredStream, PrefixedSocket, RecordingStream, RelayCarrier, StreamTraffic,
    TcpRelayStream,
};

// Re-export transport implementations from zero-transport.
// Only items used directly by proxy code are listed.
#[cfg(feature = "hysteria2")]
pub(crate) use zero_transport::hysteria2_quic::{
    open_quic_connection as open_hysteria2_quic_connection, Hysteria2QuicProfile, Hysteria2Stream,
    QuicConnectionOptions,
};
#[cfg(feature = "transport_quic")]
pub(crate) use zero_transport::quic::{QuicInbound, QuicStream};
