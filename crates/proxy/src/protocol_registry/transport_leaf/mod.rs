#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod tcp;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod udp;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use tcp::{prepare_transport_bridge_tcp_connect, prepare_transport_bridge_tcp_relay};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use udp::{
    prepare_owned_transport_bridge_udp_relay_final_hop, prepare_transport_bridge_udp_direct,
};
#[cfg(feature = "vless")]
pub(crate) use udp::{
    prepare_owned_transport_bridge_udp_relay_two_stream,
    transport_bridge_udp_relay_needs_two_streams,
};
