use super::model::{ManagedUdpHandlers, ManagedUdpState};

impl ManagedUdpState {
    pub(crate) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            #[cfg(feature = "managed-datagram-runtime")]
            datagram: super::super::datagram::ManagedDatagramState::new(handlers.datagram),
            #[cfg(feature = "managed-stream-runtime")]
            stream: super::super::stream::ManagedStreamState::new(
                #[cfg(feature = "managed-stream-runtime")]
                handlers.stream_packet,
                handlers.relay,
            ),
        }
    }
}
