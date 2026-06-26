//! Protocol-specific UDP runtime managers.
//!
//! Generic UDP dispatch owns flow lifecycle and adapter dispatch. Concrete
//! protocol managers live here and are called through request structs.

mod flow_snapshot;
mod flows;
#[cfg(feature = "hysteria2")]
mod h2_manager;
#[cfg(feature = "mieru")]
mod mieru_manager;
#[cfg(feature = "shadowsocks")]
mod ss_manager;
mod start;
mod state;
#[cfg(feature = "trojan")]
mod trojan_manager;
#[cfg(feature = "vless")]
pub(crate) mod vless_flow;
#[cfg(feature = "vmess")]
pub(crate) mod vmess_flow;

pub(crate) use crate::runtime::udp_dispatch::FlowFailure;
pub(crate) use flow_snapshot::{ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot};
pub(crate) use flows::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
pub(crate) use state::ProtocolUdpState;
