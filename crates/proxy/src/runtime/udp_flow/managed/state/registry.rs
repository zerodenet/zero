use super::model::{ManagedUdpHandlers, ManagedUdpState};

impl ManagedUdpState {
    pub(crate) fn new(handlers: ManagedUdpHandlers) -> Self {
        Self {
            #[cfg(feature = "managed-datagram-runtime")]
            datagram: super::super::datagram::ManagedDatagramState::new(handlers.datagram),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            stream: super::super::stream::ManagedStreamState::new(
                #[cfg(any(
                    feature = "vless",
                    feature = "vmess",
                    feature = "trojan",
                    feature = "mieru"
                ))]
                handlers.stream_packet,
                handlers.relay,
            ),
        }
    }
}
