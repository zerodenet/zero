#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod error;
mod stream_packet;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
mod transport;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use stream_packet::managed_stream_udp_handler_for_bridge;
#[cfg(feature = "mieru")]
pub(crate) use stream_packet::{
    managed_stream_handler_box, start_direct_managed_stream_packet,
    start_relay_managed_stream_packet, ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
    ManagedStreamStages,
};
#[cfg(feature = "vless")]
pub(crate) use transport::{
    protocol_transport_bridge_udp_relay_needs_two_streams,
    start_protocol_transport_bridge_udp_relay_two_stream,
};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use transport::{
    start_protocol_transport_bridge_udp_flow, start_protocol_transport_bridge_udp_relay_final_hop,
};
