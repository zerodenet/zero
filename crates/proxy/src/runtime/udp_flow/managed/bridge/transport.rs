mod direct;
mod relay;
#[cfg(feature = "vless")]
mod two_stream;

pub(crate) use direct::start_protocol_transport_bridge_udp_flow;
pub(crate) use relay::start_protocol_transport_bridge_udp_relay_final_hop;
#[cfg(feature = "vless")]
pub(crate) use two_stream::{
    protocol_transport_bridge_udp_relay_needs_two_streams,
    start_protocol_transport_bridge_udp_relay_two_stream,
};
