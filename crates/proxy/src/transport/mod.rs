mod direct;
mod metered;
mod stream;
mod tcp_flow;
mod tcp_outbound;
mod tcp_relay;
pub(crate) mod tls_hello;

pub(crate) use direct::DirectConnector;
pub(crate) use metered::{MeteredStream, StreamTraffic};
pub(crate) use stream::{ClientStream, PrefixedSocket, RelayCarrier, TcpRelayStream};
pub(crate) use tcp_flow::is_block_error;
pub(crate) use tcp_outbound::{
    extract_tcp_stream, EstablishedTcpOutbound, TcpOutboundFailure, TcpRouteResult,
};
pub(crate) use tcp_relay::{
    copy_one_way, relay_bidirectional_metered, relay_bidirectional_metered_throttled,
};

// Re-export transport implementations from zero-transport.
// Only items used directly by proxy code are listed.
#[cfg(feature = "vless")]
pub(crate) use zero_transport::grpc::serve_grpc;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::h2::accept_h2;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::http_upgrade::accept_http_upgrade;
#[cfg(feature = "hysteria2")]
pub(crate) use zero_transport::hysteria2_quic::{
    establish_hysteria2_udp_flow_stream, Hysteria2Connector, Hysteria2Stream,
    Hysteria2UdpFlowStreamRequest,
};
#[cfg(feature = "vless")]
pub(crate) use zero_transport::quic::{connect_quic, QuicInbound};
#[cfg(feature = "vless")]
pub(crate) use zero_transport::split_http::{
    accept_split_http, accept_xhttp_stream_one, SplitHttpRegistry,
};
#[cfg(any(feature = "vless", feature = "trojan", feature = "vmess"))]
pub(crate) use zero_transport::tls::build_tls_acceptor;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::tls::InboundTlsStream;
#[cfg(any(feature = "trojan", feature = "vmess"))]
pub(crate) use zero_transport::tls::TlsAcceptor;
#[cfg(feature = "trojan")]
pub(crate) use zero_transport::trojan_transport::{
    open_trojan_udp_tls_relay_stream, open_trojan_udp_tls_stream, TrojanUdpTlsOptions,
};
#[cfg(feature = "vless")]
pub(crate) use zero_transport::vless_transport::build_vless_split_http_over_relay;
#[cfg(feature = "vless")]
pub(crate) use zero_transport::vless_transport::{
    build_vless_outbound_transport_over_stream, VlessFinalHopTransportRequest,
    VlessTransportConnector, VlessTransportOptions, VlessUdpTransportConnector,
    VlessUdpTransportOptions,
};
#[cfg(feature = "vmess")]
pub(crate) use zero_transport::vmess_transport::{
    build_vmess_outbound_transport_over_stream, VmessFinalHopTransportRequest,
    VmessTransportConnector, VmessTransportOptions,
};
#[cfg(feature = "vless")]
pub(crate) use zero_transport::ws::accept_ws;
