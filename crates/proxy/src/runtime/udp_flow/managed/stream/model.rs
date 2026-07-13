use super::super::model::ManagedRelayFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use super::super::model::ManagedStreamPacketFlowHandler;

pub(in crate::runtime::udp_flow::managed) struct ManagedStreamState {
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(in crate::runtime::udp_flow::managed) stream_packet_handlers:
        Vec<Box<dyn ManagedStreamPacketFlowHandler>>,
    pub(in crate::runtime::udp_flow::managed) relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>,
}

impl ManagedStreamState {
    pub(in crate::runtime::udp_flow::managed) fn new(
        #[cfg(any(
            feature = "vless",
            feature = "vmess",
            feature = "trojan",
            feature = "mieru"
        ))]
        stream_packet_handlers: Vec<Box<dyn ManagedStreamPacketFlowHandler>>,
        relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>,
    ) -> Self {
        Self {
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            stream_packet_handlers,
            relay_handlers,
        }
    }
}
