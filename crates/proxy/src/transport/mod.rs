mod direct;
mod tcp_flow;
mod tcp_outbound;
mod tcp_relay;

pub(crate) use direct::DirectConnector;
pub(crate) use tcp_flow::is_block_error;
pub(crate) use tcp_outbound::{
    apply_protocol_transport_bridge_relay_hop, connect_protocol_transport_bridge_tcp,
    extract_tcp_stream, EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult,
};
pub(crate) use tcp_relay::{relay_bidirectional_metered, relay_bidirectional_metered_throttled};
pub(crate) use zero_transport::{
    ClientStream, MeteredStream, PrefixedSocket, RecordingStream, RelayCarrier, StreamTraffic,
    TcpRelayStream,
};

// Re-export transport implementations from zero-transport.
// Only items used directly by proxy code are listed.
#[cfg(feature = "transport_quic")]
pub(crate) use zero_transport::quic::{QuicInbound, QuicStream};
