mod stream_packet;

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use stream_packet::managed_stream_udp_handler_for_resume;
#[cfg(feature = "mieru")]
pub(crate) use stream_packet::{managed_stream_handler_box, ManagedStreamStages};
pub(crate) use stream_packet::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
