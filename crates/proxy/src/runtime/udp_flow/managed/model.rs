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

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use handler::ManagedDatagramFlowHandler;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use handler::ManagedRelayFlowHandler;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use handler::ManagedStreamHandlerPair;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use handler::ManagedStreamPacketFlowHandler;
#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use send::ManagedDatagramExistingSend;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use send::ManagedRelayExistingSend;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use send::ManagedStreamExistingSend;
