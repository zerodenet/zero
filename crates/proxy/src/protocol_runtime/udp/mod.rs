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
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod start;
mod state;
#[cfg(feature = "trojan")]
mod trojan_manager;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "shadowsocks")]
pub(crate) use flow_snapshot::PacketPathFlowSnapshot;
pub(crate) use flow_snapshot::ProtocolUdpFlowSnapshot;
#[cfg(feature = "mieru")]
pub(crate) use flows::MieruUdpRelayFlow;
#[cfg(feature = "shadowsocks")]
pub(crate) use flows::ShadowsocksUdpFlow;
#[cfg(feature = "vless")]
pub(crate) use flows::{VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream};
#[cfg(feature = "vmess")]
pub(crate) use flows::{VmessUdpFlow, VmessUdpRelayFlow};
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_chain::{PacketPathManager, SendWithSnapshotRequest};
pub(crate) use packet_path_traits::ChainTask;
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_traits::{
    PacketPathCarrier, PacketPathCarrierDescriptor, PacketPathCarrierSnapshot,
    UdpDatagramDescriptor, UdpDatagramSource,
};
#[cfg(feature = "hysteria2")]
pub(crate) use start::Hysteria2UdpFlowRequest;
#[cfg(feature = "mieru")]
pub(crate) use start::MieruUdpFlowRequest;
#[cfg(feature = "trojan")]
pub(crate) use start::{TrojanUdpFlowRequest, TrojanUdpRelayFlowRequest};
pub(crate) use state::ProtocolUdpState;
