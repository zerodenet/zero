//! Packet path chain abstractions for UDP relay chains.
//!
//! Concrete protocol managers import these through this facade; the grouped
//! definitions live under `packet_path_traits/` by responsibility.

#[cfg(feature = "shadowsocks")]
mod carrier;
mod context;
mod peer;

#[cfg(feature = "shadowsocks")]
pub(crate) use carrier::{
    DatagramCodec, PacketPathCarrier, PacketPathCarrierDescriptor, UdpDatagramSource,
};
pub(crate) use context::{ChainTask, UdpFlowContext, UdpPacketRef};
#[cfg(feature = "hysteria2")]
pub(crate) use peer::H2UdpPeer;
#[cfg(feature = "mieru")]
pub(crate) use peer::MieruUdpPeer;
#[cfg(feature = "shadowsocks")]
pub(crate) use peer::SsUdpPeer;
#[cfg(feature = "trojan")]
pub(crate) use peer::TrojanUdpPeer;
pub(crate) use peer::UdpPeerEndpoint;
