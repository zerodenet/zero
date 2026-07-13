use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::ProtocolRegistry;
use crate::protocol_registry::{OutboundLeafRuntime, TcpOutboundCapability};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::protocol_registry::{UdpFlowCapability, UdpPacketPathCapability};
use crate::runtime::path::TcpPathCategory;

pub(crate) fn direct_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<OutboundLeafRuntime<'a>> {
    match leaf {
        ResolvedLeafOutbound::Direct { tag } => Some(OutboundLeafRuntime {
            tcp_path: TcpPathCategory::Direct,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            health_tag: None,
            endpoint: None,
            kernel_tag: *tag,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp_policy_tag: *tag,
        }),
        _ => None,
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) fn proxy_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
    tcp_path: TcpPathCategory,
) -> Option<OutboundLeafRuntime<'a>> {
    let tag = leaf.tag()?;
    let (server, port) = leaf.proxy_endpoint()?;

    Some(OutboundLeafRuntime {
        tcp_path,
        #[cfg(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        ))]
        health_tag: Some(tag),
        endpoint: Some(crate::runtime::path::OutboundEndpoint { server, port }),
        kernel_tag: None,
        #[cfg(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        ))]
        udp_policy_tag: Some(tag),
    })
}

impl ProtocolRegistry {
    /// Find the adapter that owns this resolved outbound leaf, if any.
    ///
    /// Single dispatch point: the TCP/UDP runtime resolves a
    /// [`ResolvedLeafOutbound`] to its adapter here instead of matching on
    /// the protocol enum. Each adapter claims exactly its own variant via
    /// [`TcpOutboundCapability::claims_outbound_leaf`].
    pub(crate) fn find_outbound_leaf(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn TcpOutboundCapability>, EngineError> {
        for entry in &self.entries {
            if entry.tcp.claims_outbound_leaf(leaf) {
                return Ok(entry.tcp.clone());
            }
        }
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter handles this outbound leaf",
        )))
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn find_udp_flow_leaf(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn UdpFlowCapability>, EngineError> {
        for entry in &self.entries {
            if entry.tcp.claims_outbound_leaf(leaf) {
                return entry.udp.clone().ok_or_else(unsupported_outbound_leaf);
            }
        }
        Err(unsupported_outbound_leaf())
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn find_udp_packet_path_leaf(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn UdpPacketPathCapability>, EngineError> {
        for entry in &self.entries {
            if entry.tcp.claims_outbound_leaf(leaf) {
                return entry
                    .packet_path
                    .clone()
                    .ok_or_else(unsupported_outbound_leaf);
            }
        }
        Err(unsupported_outbound_leaf())
    }

    /// Return neutral runtime facts for a resolved outbound leaf.
    ///
    /// Kernel-level `block` is handled here because no adapter owns it.
    /// Direct and proxy protocols are delegated to the adapter that claims the
    /// leaf, so runtime code does not match protocol variants.
    pub(crate) fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundLeafRuntime<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf {
            return Ok(OutboundLeafRuntime {
                tcp_path: TcpPathCategory::Block,
                #[cfg(any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                ))]
                health_tag: None,
                endpoint: None,
                kernel_tag: *tag,
                #[cfg(any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                ))]
                udp_policy_tag: *tag,
            });
        }

        for entry in &self.entries {
            if !entry.tcp.claims_outbound_leaf(leaf) {
                continue;
            }
            if let Some(runtime) = entry.tcp.outbound_leaf_runtime(leaf) {
                return Ok(runtime);
            }
            break;
        }

        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter describes this outbound leaf",
        )))
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn unsupported_outbound_leaf() -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no compiled adapter handles this outbound leaf",
    ))
}
