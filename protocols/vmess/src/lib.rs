mod crypto;
mod inbound;
mod metadata;
mod mux;
mod outbound;
mod shared;
mod stream;
mod udp;

pub use inbound::{VmessAccept, VmessInbound, VmessUser};
pub use metadata::VmessProtocol;
pub use mux::{
    decode_metadata as decode_mux_metadata, encode_end_stream as encode_mux_end_stream,
    encode_frame as encode_mux_frame, encode_keep_stream as encode_mux_keep_stream,
    encode_open_stream as encode_mux_open_stream, is_mux_cool_session, mux_cool_session,
    read_frame as read_mux_frame, read_frame_from_tokio as read_mux_frame_from_tokio, MuxFrame,
    VmessMuxStream, MUX_MAX_DATA_LEN, MUX_MAX_META_LEN, MUX_NETWORK_TCP, MUX_NETWORK_UDP,
    MUX_OPTION_DATA, MUX_OPTION_ERROR, MUX_STATUS_END, MUX_STATUS_KEEP, MUX_STATUS_KEEP_ALIVE,
    MUX_STATUS_NEW,
};
pub use outbound::{VmessOutbound, VmessOutboundSession, VmessTcpSessionTarget};
pub use shared::{
    parse_uuid, VmessCipher, AUTH_ID_LEN, CMD_TCP, CMD_UDP, GCM_TAG_LEN, MUX_COOL_DOMAIN,
    MUX_COOL_PORT, VERSION,
};
pub use stream::VmessAeadStream;
pub use udp::{
    build_udp_packet, decode_inbound_udp_payload, encode_mux_udp_response, encode_udp_response,
    establish_udp_outbound_stream, parse_udp_packet, VmessInboundUdpPayload, VmessUdpPacket,
    VmessUdpPacketTarget, VmessUdpPacketTunnelTarget, VmessUdpPayloadMode, VmessUdpPayloadState,
};
