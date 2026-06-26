//! Protocol-specific UDP runtime managers.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. Concrete
//! protocol managers live here and are called through request structs.

pub(crate) mod packet_path_traits;

mod flow_snapshot;
mod flows;
#[cfg(feature = "hysteria2")]
mod h2_manager;
#[cfg(feature = "mieru")]
mod mieru_manager;
#[cfg(feature = "shadowsocks")]
pub(crate) mod packet_path_chain;
pub(crate) mod packet_path_snapshot;
mod peer;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod start;
mod state;
#[cfg(feature = "trojan")]
mod trojan_manager;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
pub(crate) use flow_snapshot::{ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot};
#[cfg(feature = "mieru")]
pub(crate) use flows::MieruUdpRelayFlow;
pub(crate) use flows::{ManagedDatagramFlow, ManagedStreamPacketFlow};
#[cfg(feature = "vless")]
pub(crate) use flows::{VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream};
#[cfg(feature = "vmess")]
pub(crate) use flows::{VmessUdpFlow, VmessUdpRelayFlow};
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_chain::{PacketPathManager, SendWithSnapshotRequest};
pub(crate) use packet_path_traits::ChainTask;
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_traits::{
    PacketPathCarrier, PacketPathCarrierDescriptor, PacketPathFlowBinding, PacketPathFlowSnapshot,
    PacketPathLookupKey, UdpDatagramDescriptor, UdpDatagramSource,
};
#[cfg(feature = "hysteria2")]
pub(crate) use peer::H2UdpPeer;
#[cfg(feature = "mieru")]
pub(crate) use peer::MieruUdpPeer;
#[cfg(feature = "shadowsocks")]
pub(crate) use peer::SsUdpPeer;
#[cfg(feature = "trojan")]
pub(crate) use peer::TrojanUdpPeer;
pub(crate) use peer::UdpPeerEndpoint;
#[cfg(feature = "trojan")]
pub(crate) use start::TrojanUdpRelayFlowRequest;
pub(crate) use state::ProtocolUdpState;
