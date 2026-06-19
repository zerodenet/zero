//! Concrete `ProtocolAdapter` implementations for each compiled-in protocol.

use std::sync::Arc;

use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_adapter::{BoundInbound, ProtocolAdapter};
use crate::protocol_capability::protocol_descriptor;
use crate::runtime::udp_associate::sessions::UdpFlowOutbound;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
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
use direct::DirectAdapter;
#[cfg(feature = "http_connect")]
use http_connect::HttpConnectAdapter;
#[cfg(feature = "hysteria2")]
use hysteria2::Hysteria2Adapter;
#[cfg(feature = "mieru")]
use mieru::MieruAdapter;
#[cfg(feature = "shadowsocks")]
use shadowsocks::ShadowsocksAdapter;
#[cfg(feature = "socks5")]
use socks5::Socks5Adapter;
#[cfg(feature = "trojan")]
use trojan::TrojanAdapter;
#[cfg(feature = "vless")]
use vless::VlessAdapter;
#[cfg(feature = "vmess")]
use vmess::VmessAdapter;

pub(crate) fn build_registry() -> super::protocol_adapter::ProtocolRegistry {
    let mut r = super::protocol_adapter::ProtocolRegistry::default();

    #[cfg(feature = "socks5")]
    r.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    r.register(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    r.register(Arc::new(VlessAdapter));
    #[cfg(feature = "hysteria2")]
    r.register(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    r.register(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    r.register(Arc::new(TrojanAdapter));
    #[cfg(feature = "vmess")]
    r.register(Arc::new(VmessAdapter));
    #[cfg(feature = "mieru")]
    r.register(Arc::new(MieruAdapter));
    r.register(Arc::new(DirectAdapter));

    r
}
