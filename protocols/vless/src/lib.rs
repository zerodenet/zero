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
pub mod udp;

#[cfg(feature = "reality")]
pub use deferred_response::DeferredVlessResponseStream;
#[cfg(feature = "reality")]
pub use flow::{
    flow_build_request, flow_byte, flow_from_byte, parse_flow, FLOW_XTLS_RPRX_VISION,
    FLOW_XTLS_RPRX_VISION_UDP,
};
pub use inbound::{
    classify_inbound_session, IntoVlessInboundUserConfig, VlessConfiguredUser,
    VlessConfiguredUsers, VlessInbound, VlessInboundProfile, VlessInboundSessionKind,
    VlessInboundUserConfigParts, VlessUser, VlessUserStore,
};
pub use metadata::VlessProtocol;
#[cfg(feature = "reality")]
pub use mux_crypto::MuxCrypto;
#[cfg(feature = "reality")]
pub use outbound::VlessFlowTcpTunnelTarget;
pub use outbound::{tcp_connect_config_from_config, VlessOutbound};
pub use outbound::{VlessTcpConnectConfig, VlessTcpTunnelTarget};
#[cfg(feature = "reality")]
pub use reality::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    RealityClientOptions, RealityServerOptions, RealityTlsStream, VlessRealityServerProfile,
};
pub use shared::{format_uuid, parse_uuid, VlessInboundUdpClientResponse, VLESS_VERSION};
