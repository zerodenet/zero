#![cfg_attr(not(feature = "reality"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

#[cfg(feature = "reality")]
mod deferred_response;
mod inbound;
mod outbound;
#[cfg(feature = "reality")]
mod reality;
mod shared;

#[cfg(feature = "reality")]
pub use deferred_response::DeferredVlessResponseStream;
pub use inbound::{VlessInbound, VlessUser, VlessUserStore};
pub use outbound::VlessOutbound;
#[cfg(feature = "reality")]
pub use reality::{
    generate_reality_key_pair, upgrade_reality_client, upgrade_reality_server,
    RealityClientOptions, RealityServerOptions, RealityTlsStream,
};
pub use shared::{build_udp_packet, format_uuid, parse_uuid, parse_udp_packet, VlessUdpPacket, VLESS_VERSION};
