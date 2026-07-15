#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod tcp;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod udp;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use tcp::claim_transport_bridge_tcp_leaf;
#[cfg(feature = "vless")]
pub(crate) use udp::claim_relay_two_stream_transport_bridge_udp_leaf;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use udp::claim_transport_bridge_udp_leaf;
