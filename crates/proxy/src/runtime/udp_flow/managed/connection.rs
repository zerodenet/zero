mod model;
#[cfg(feature = "trojan")]
mod packet;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
mod response;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "mieru",
    feature = "hysteria2"
))]
mod tuple;

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
pub(crate) use model::SharedManagedUdpConnection;
#[cfg(feature = "shadowsocks")]
pub(crate) use model::{ManagedDatagramUdpConnection, SharedManagedDatagramUdpConnection};
#[cfg(feature = "trojan")]
pub(crate) use packet::managed_packet_udp_connection_from_flow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "mieru",
    feature = "hysteria2"
))]
pub(crate) use tuple::managed_tuple_udp_connection_from_ops;
