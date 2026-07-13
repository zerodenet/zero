#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod handler;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod send;

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use handler::ManagedDatagramFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use handler::ManagedRelayFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use handler::ManagedStreamHandlerPair;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use handler::ManagedStreamPacketFlowHandler;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use send::ManagedDatagramExistingSend;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use send::ManagedRelayExistingSend;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use send::ManagedStreamExistingSend;
