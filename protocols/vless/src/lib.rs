#![cfg_attr(not(feature = "reality"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

#[cfg(feature = "reality")]
mod deferred_response;
#[cfg(feature = "reality")]
mod flow;
mod inbound;
pub mod mux;
#[cfg(feature = "reality")]
mod mux_crypto;
#[cfg(feature = "reality")]
pub mod mux_pool;
mod outbound;
#[cfg(feature = "reality")]
mod reality;
mod shared;
#[cfg(feature = "reality")]
mod udp;
#[cfg(feature = "reality")]
mod transport;

#[cfg(feature = "reality")]
pub use deferred_response::DeferredVlessResponseStream;
#[cfg(feature = "reality")]
pub use flow::{
    flow_build_request, flow_byte, flow_from_byte, parse_flow, FLOW_XTLS_RPRX_VISION,
    FLOW_XTLS_RPRX_VISION_UDP,
};
pub use inbound::{VlessInbound, VlessUser, VlessUserStore};
#[cfg(feature = "reality")]
pub use inbound::ConfiguredVlessUsers;
pub use mux::{
    encode_frame, encode_new_stream, encode_new_stream_response, parse_new_stream_payload,
    parse_new_stream_response, MuxClient, MuxClientStream, MuxFrame, MuxServer,
    MUX_FRAME_HEADER_LEN, MUX_MAX_PAYLOAD, MUX_STATUS_FAIL, MUX_STATUS_OK, MUX_STREAM_NEW,
};
#[cfg(feature = "reality")]
pub use mux_crypto::MuxCrypto;
pub use outbound::VlessOutbound;
#[cfg(feature = "reality")]
pub use reality::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    upgrade_reality_server_from_config, RealityClientOptions, RealityServerOptions,
    RealityTlsStream,
};
pub use shared::{
    build_udp_packet, build_udp_packet_v2, format_uuid, parse_udp_packet, parse_udp_packet_v2,
    parse_uuid, VlessUdpPacket, VLESS_VERSION,
};
#[cfg(feature = "reality")]
pub use udp::{VlessUdpTransport, VlessUdpUpstream};
#[cfg(feature = "reality")]
pub use transport::{
    grpc::{accept_grpc, connect_grpc, GrpcStream},
    h2::{accept_h2, connect_h2, H2Stream},
    http_upgrade::{accept_http_upgrade, connect_http_upgrade, HttpUpgradeStream},
    quic::{connect_quic, QuicInbound, QuicStream},
    tls::{build_tls_acceptor, connect_tls_upstream, InboundTlsStream},
    vless_transport::{build_vless_outbound_transport, VlessTransportConnector},
    ws::{accept_ws, connect_ws, WebSocketSocket},
};
