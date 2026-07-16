//! Prepared UDP operation contracts plus focused executor modules.
//!
//! The root stays as a facade so direct, managed-datagram, registered, and
//! managed-stream-packet execution do not regrow into one large bucket.

mod contract;
mod direct;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod managed_datagram;
#[cfg(feature = "socks5")]
mod registered;
#[cfg(feature = "mieru")]
mod stream_packet;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) mod transport;

pub(crate) use contract::PreparedUdpFlowOperation;
pub(crate) use direct::DirectUdpFlowOperation;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use managed_datagram::ManagedDatagramUdpOperation;
#[cfg(feature = "socks5")]
pub(crate) use registered::RegisteredAssociationUdpOperation;
#[cfg(feature = "mieru")]
pub(crate) use stream_packet::{
    ManagedStreamPacketUdpOperation, PreparedManagedStreamPacketOperation,
};
