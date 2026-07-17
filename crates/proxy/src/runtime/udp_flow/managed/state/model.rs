#[cfg(feature = "managed-datagram-runtime")]
use super::super::datagram::ManagedDatagramState;
#[cfg(feature = "managed-datagram-runtime")]
use super::super::model::ManagedDatagramFlowHandler;
#[cfg(feature = "managed-stream-runtime")]
use super::super::model::{ManagedRelayFlowHandler, ManagedStreamPacketFlowHandler};
#[cfg(feature = "managed-stream-runtime")]
use super::super::stream::ManagedStreamState;

pub(crate) struct ManagedUdpHandlers {
    #[cfg(feature = "managed-datagram-runtime")]
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
    #[cfg(feature = "managed-datagram-runtime")]
    pub(super) datagram: ManagedDatagramState,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(super) stream: ManagedStreamState,
}
