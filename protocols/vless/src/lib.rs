#![cfg_attr(not(feature = "reality"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

#[cfg(feature = "reality")]
mod deferred_response;
#[cfg(feature = "reality")]
mod flow;
mod inbound;
pub mod metadata;
pub mod mux;
#[cfg(feature = "reality")]
mod mux_crypto;
#[cfg(feature = "reality")]
pub mod mux_pool;
mod outbound;
#[cfg(feature = "reality")]
pub mod reality;
mod shared;

#[cfg(feature = "reality")]
pub use deferred_response::DeferredVlessResponseStream;
#[cfg(feature = "reality")]
pub use flow::{
    flow_build_request, flow_byte, flow_from_byte, parse_flow, FLOW_XTLS_RPRX_VISION,
    FLOW_XTLS_RPRX_VISION_UDP,
};
pub use inbound::{VlessInbound, VlessUser, VlessUserStore};
pub use metadata::VlessProtocol;
pub use mux::{
    encode_data_frame, encode_end_frame, encode_frame, encode_keepalive, encode_new_stream,
    encode_new_stream_response, encode_udp_data_frame, parse_new_stream, parse_new_stream_response,
    parse_udp_target_from_keep, MuxClient, MuxClientStream, MuxFrame, MuxServer, MuxTarget,
    MUX_FRAME_HEADER_LEN, MUX_MAX_PAYLOAD, MUX_NETWORK_TCP, MUX_NETWORK_UDP, MUX_STATUS_FAIL,
    MUX_STATUS_OK, MUX_STREAM_NEW, NETWORK_TCP, NETWORK_UDP, OPTION_DATA, STATUS_END, STATUS_KEEP,
    STATUS_KEEP_ALIVE, STATUS_NEW,
};
#[cfg(feature = "reality")]
pub use mux_crypto::MuxCrypto;
#[cfg(feature = "reality")]
pub use outbound::VlessFlowTcpTunnelTarget;
pub use outbound::{
    VlessOutbound, VlessTcpTunnelTarget, VlessUdpPacketTarget, VlessUdpPacketTunnelTarget,
};
#[cfg(feature = "reality")]
pub use reality::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    RealityClientOptions, RealityServerOptions, RealityTlsStream,
};
pub use shared::{
    build_udp_packet, build_udp_packet_v2, format_uuid, parse_udp_packet, parse_udp_packet_v2,
    parse_uuid, VlessUdpPacket, VLESS_VERSION,
};
