#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

mod inbound;
mod metadata;
mod outbound;
pub mod shared;
pub mod udp;

#[cfg(feature = "crypto")]
pub use inbound::Hysteria2InboundProfile;
pub use inbound::{Hysteria2Inbound, Hysteria2User, Hysteria2UserStore};
pub use metadata::Hysteria2Protocol;
pub use outbound::Hysteria2Outbound;
pub use shared::{
    build_auth_error, build_auth_frame, build_auth_ok, build_connect_error, build_connect_ok,
    build_tcp_connect_header, parse_auth_frame, parse_auth_response, parse_tcp_connect_header,
    ADDR_TYPE_DOMAIN, ADDR_TYPE_IPV4, ADDR_TYPE_IPV6, AUTH_ERR, AUTH_OK, HYSTERIA2_VERSION,
    STREAM_TYPE_TCP, STREAM_TYPE_UDP,
};
#[cfg(feature = "crypto")]
pub use shared::{derive_salt, sign_hmac, verify_hmac};
#[cfg(feature = "tokio")]
pub use udp::{
    spawn_udp_flow, start_udp_flow_with_initial_packet, Hysteria2InboundUdpSession,
    Hysteria2InitialUdpFlowPacket, Hysteria2UdpFlowConnection, Hysteria2UdpFlowHandle,
    Hysteria2UdpFlowResponse, Hysteria2UdpFlowResponseReceiver, Hysteria2UdpFlowSession,
    Hysteria2UdpFlowSessions,
};
pub use udp::{
    udp_flow_resume_from_config, udp_packet_path_spec_from_config, Hysteria2DatagramCodec,
    Hysteria2InboundUdpCodec, Hysteria2InboundUdpRequest, Hysteria2UdpConnectorProfile,
    Hysteria2UdpFlowConfig, Hysteria2UdpFlowIo, Hysteria2UdpFlowPacket, Hysteria2UdpFlowResume,
    Hysteria2UdpFlowStore, Hysteria2UdpPacket, Hysteria2UdpPacketPathSpec,
    Hysteria2UdpPacketTarget,
};
