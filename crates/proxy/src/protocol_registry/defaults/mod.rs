mod bind;
mod errors;

pub(super) use bind::bind_tcp_inbound;
#[cfg(feature = "transport_quic")]
pub(crate) use bind::bind_transport_inbound;
pub(crate) use errors::unreachable_leaf;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use errors::unreachable_udp_leaf;
pub(super) use errors::{
    relay_hop_unsupported, tcp_outbound_unsupported, udp_outbound_unsupported,
    udp_relay_final_hop_unsupported, udp_two_stream_relay_unsupported,
};
