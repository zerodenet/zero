use super::super::model::ManagedRelayFlowHandler;
#[cfg(feature = "managed-stream-runtime")]
use super::super::model::ManagedStreamPacketFlowHandler;

pub(in crate::runtime::udp_flow::managed) struct ManagedStreamState {
    #[cfg(feature = "managed-stream-runtime")]
    pub(in crate::runtime::udp_flow::managed) stream_packet_handlers:
        Vec<Box<dyn ManagedStreamPacketFlowHandler>>,
    pub(in crate::runtime::udp_flow::managed) relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>,
}

impl ManagedStreamState {
    pub(in crate::runtime::udp_flow::managed) fn new(
        #[cfg(feature = "managed-stream-runtime")] stream_packet_handlers: Vec<
            Box<dyn ManagedStreamPacketFlowHandler>,
        >,
        relay_handlers: Vec<Box<dyn ManagedRelayFlowHandler>>,
    ) -> Self {
        Self {
            #[cfg(feature = "managed-stream-runtime")]
            stream_packet_handlers,
            relay_handlers,
        }
    }
}
