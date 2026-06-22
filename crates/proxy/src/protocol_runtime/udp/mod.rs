//! Protocol-specific UDP runtime managers.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. Concrete
//! protocol managers live here and are called through request structs.

pub(crate) mod packet_path_traits;

#[cfg(feature = "hysteria2")]
mod h2_manager;
#[cfg(feature = "mieru")]
mod mieru_manager;
#[cfg(feature = "shadowsocks")]
mod packet_path_chain;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod state;
#[cfg(feature = "trojan")]
mod trojan_manager;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "hysteria2")]
pub(crate) use h2_manager::{H2ChainManager, H2SendExisting};
#[cfg(feature = "mieru")]
pub(crate) use mieru_manager::MieruChainManager;
#[cfg(all(feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) use packet_path_chain::build_hysteria2_packet_path;
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_chain::{build_shadowsocks_packet_path, PacketPathManager};
pub(crate) use packet_path_traits::ChainTask;
#[cfg(feature = "shadowsocks")]
pub(crate) use packet_path_traits::{
    PacketPathCarrier, PacketPathCarrierDescriptor, UdpDatagramSource,
};
#[cfg(feature = "shadowsocks")]
pub(crate) use ss_manager::{SsChainManager, SsSendExisting};
pub(crate) use state::ProtocolUdpState;
#[cfg(feature = "trojan")]
pub(crate) use trojan_manager::{TrojanChainManager, TrojanRelayExisting, TrojanSendExisting};
