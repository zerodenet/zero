use super::super::model::{UdpFlowIndexKeys, UdpFlowOutbound};

#[cfg(feature = "udp-runtime")]

impl UdpFlowOutbound {
    pub(in crate::runtime::udp_flow) fn index_keys(&self) -> UdpFlowIndexKeys<'_> {
        UdpFlowIndexKeys {
            direct_sender: self.direct_target_addr(),
            upstream_response_tag: self.upstream_response_tag(),
        }
    }

    fn upstream_response_tag(&self) -> Option<&str> {
        match self {
            Self::Direct { .. } => None,
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { tag, .. } => Some(tag),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { tag, .. } => Some(tag),
            #[cfg(feature = "managed-datagram-runtime")]
            Self::Datagram { tag, .. } => Some(tag),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { tag, .. } => Some(tag),
        }
    }
}
