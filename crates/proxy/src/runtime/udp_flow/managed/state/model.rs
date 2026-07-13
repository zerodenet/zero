#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use super::super::datagram::ManagedDatagramState;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use super::super::model::ManagedDatagramFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use super::super::model::{ManagedRelayFlowHandler, ManagedStreamPacketFlowHandler};
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use super::super::stream::ManagedStreamState;

pub(crate) struct ManagedUdpHandlers {
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    pub(crate) datagram: Vec<Box<dyn ManagedDatagramFlowHandler>>,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) stream_packet: Vec<Box<dyn ManagedStreamPacketFlowHandler>>,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) relay: Vec<Box<dyn ManagedRelayFlowHandler>>,
}

pub(crate) struct ManagedUdpState {
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    pub(super) datagram: ManagedDatagramState,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(super) stream: ManagedStreamState,
}
