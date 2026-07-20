use super::super::UdpFlowOutbound;

impl UdpFlowOutbound {
    pub(crate) fn observed_remote(&self) -> (String, u16) {
        match self {
            Self::Direct { target_addr, .. } => (target_addr.ip().to_string(), target_addr.port()),
            #[cfg(any(
                feature = "upstream-association-runtime",
                feature = "managed-stream-runtime"
            ))]
            Self::Relay { server, port, .. } => (server.clone(), *port),
            #[cfg(feature = "managed-datagram-runtime")]
            Self::Datagram { server, port, .. } => (server.clone(), *port),
            #[cfg(feature = "managed-stream-runtime")]
            Self::StreamPacket { server, port, .. } => (server.clone(), *port),
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { server, port, .. } => (server.clone(), *port),
        }
    }
}
