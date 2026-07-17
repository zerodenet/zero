use crate::runtime::udp_flow::outbound::model::{UdpFlowOutbound, UdpFlowUpstream};

impl UdpFlowOutbound {
    #[cfg(feature = "udp-runtime")]
    pub(crate) fn packet_path_snapshot(
        &self,
    ) -> Option<&crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot> {
        match self {
            Self::PacketPathDatagram { snapshot, .. } => Some(snapshot),
            Self::Direct { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => None,
            #[cfg(feature = "managed-datagram-runtime")]
            Self::Datagram { .. } => None,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => None,
        }
    }

    pub(crate) fn upstream(&self) -> Option<UdpFlowUpstream<'_>> {
        match self {
            Self::Direct { .. } => None,
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(feature = "managed-datagram-runtime")]
            Self::Datagram { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
        }
    }
}
