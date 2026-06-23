//! Concrete `ProtocolAdapter` implementations for each compiled-in protocol.

use std::sync::Arc;

use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_adapter::{BoundInbound, ProtocolAdapter};
use crate::protocol_capability::protocol_descriptor;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, QuicInbound, TcpOutboundFailure};

mod common;
mod direct;
#[cfg(feature = "http_connect")]
mod http_connect;
#[cfg(feature = "hysteria2")]
mod hysteria2;
#[cfg(feature = "mieru")]
mod mieru;
#[cfg(feature = "mixed")]
mod mixed;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "socks5")]
mod socks5;
#[cfg(feature = "trojan")]
mod trojan;
#[cfg(feature = "vless")]
mod vless;
#[cfg(feature = "vmess")]
mod vmess;

use common::{unreachable_leaf, unreachable_udp_leaf};
pub(crate) use direct::DirectAdapter;
#[cfg(feature = "http_connect")]
pub(crate) use http_connect::HttpConnectAdapter;
#[cfg(feature = "hysteria2")]
pub(crate) use hysteria2::Hysteria2Adapter;
#[cfg(feature = "mieru")]
pub(crate) use mieru::MieruAdapter;
#[cfg(feature = "mixed")]
pub(crate) use mixed::MixedAdapter;
#[cfg(feature = "shadowsocks")]
pub(crate) use shadowsocks::ShadowsocksAdapter;
#[cfg(feature = "socks5")]
pub(crate) use socks5::Socks5Adapter;
#[cfg(feature = "trojan")]
pub(crate) use trojan::TrojanAdapter;
#[cfg(feature = "vless")]
pub(crate) use vless::VlessAdapter;
#[cfg(feature = "vmess")]
pub(crate) use vmess::VmessAdapter;
