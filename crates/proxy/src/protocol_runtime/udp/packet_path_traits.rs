//! Packet path chain abstractions for UDP relay chains.
//!
//! The grouped definitions live under `packet_path_traits/` by responsibility.

#[cfg(feature = "shadowsocks")]
mod carrier;
mod context;

#[cfg(feature = "shadowsocks")]
pub(crate) use carrier::{
    DatagramCodec, PacketPathCarrier, PacketPathCarrierDescriptor, PacketPathFlowBinding,
    PacketPathFlowSnapshot, PacketPathLookupKey, UdpDatagramDescriptor, UdpDatagramKey,
    UdpDatagramSource,
};
pub(crate) use context::{ChainTask, UdpFlowContext, UdpPacketRef};
