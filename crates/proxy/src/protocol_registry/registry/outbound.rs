use std::path::Path;
use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{ProtocolRegistry, RegisteredProtocolEntry};
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
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::transport::RelayCarrier;
#[derive(Clone)]
pub(crate) struct ClaimedOutboundLeaf<'a> {
    leaf: ResolvedLeafOutbound<'a>,
    pub(crate) runtime: OutboundLeafRuntime,
    pub(crate) tcp: Option<Arc<dyn TcpOutboundCapability>>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) udp: Option<Arc<dyn UdpFlowCapability>>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) packet_path: Option<Arc<dyn UdpPacketPathCapability>>,
}

impl<'a> ClaimedOutboundLeaf<'a> {
    fn new(
        leaf: ResolvedLeafOutbound<'a>,
        runtime: OutboundLeafRuntime,
        entry: Option<&RegisteredProtocolEntry>,
    ) -> Self {
        Self {
            leaf,
            runtime,
            tcp: entry.map(|entry| entry.tcp.clone()),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp: entry.and_then(|entry| entry.udp.clone()),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            packet_path: entry.and_then(|entry| entry.packet_path.clone()),
        }
    }

    pub(crate) fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    {
        self.tcp
            .as_ref()
            .expect("non-block tcp leaf must expose a tcp capability")
            .prepare_tcp_connect(self.leaf.clone(), source_dir)
    }

    pub(crate) fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<(String, u16, Box<dyn PreparedTcpRelayOperation + 'a>), EngineError> {
        let endpoint = self.runtime.endpoint.clone().ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "relay hop resolved without upstream endpoint",
            ))
        })?;
        let operation = self
            .tcp
            .as_ref()
            .expect("tcp relay hop must expose a tcp capability")
            .prepare_tcp_relay_hop(self.leaf.clone(), source_dir)?;
        Ok((endpoint.server, endpoint.port, operation))
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
    pub(crate) fn prepare_udp_flow(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        self.udp
            .as_ref()
            .expect("non-block udp leaf must expose a udp-flow capability")
            .prepare_udp_flow(self.leaf.clone(), source_dir)
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
    pub(crate) fn udp_relay_needs_two_streams(&self, source_dir: Option<&Path>) -> bool {
        self.udp
            .as_ref()
            .expect("udp relay leaf must expose a udp-flow capability")
            .udp_relay_needs_two_streams(&self.leaf, source_dir)
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
    pub(crate) fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        self.udp
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?
            .prepare_owned_udp_relay_final_hop(carrier, self.leaf.clone(), source_dir)
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
    pub(crate) fn prepare_owned_udp_relay_two_stream(
        &self,
        post_carrier: RelayCarrier,
        get_carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        self.udp
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?
            .prepare_owned_udp_relay_two_stream(
                post_carrier,
                get_carrier,
                self.leaf.clone(),
                source_dir,
            )
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
    pub(crate) fn prepare_udp_packet_path(
        &self,
    ) -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    > {
        self.packet_path
            .as_ref()?
            .prepare_udp_packet_path(self.leaf.clone())
    }
}

pub(crate) fn direct_leaf_runtime(leaf: &ResolvedLeafOutbound<'_>) -> Option<OutboundLeafRuntime> {
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
            kernel_tag: (*tag).map(str::to_owned),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp_policy_tag: (*tag).map(str::to_owned),
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
pub(crate) fn proxy_leaf_runtime(
    leaf: &ResolvedLeafOutbound<'_>,
    tcp_path: TcpPathCategory,
) -> Option<OutboundLeafRuntime> {
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
        health_tag: Some(tag.to_owned()),
        endpoint: Some(crate::runtime::path::OutboundEndpoint {
            server: server.to_owned(),
            port,
        }),
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
        udp_policy_tag: Some(tag.to_owned()),
    })
}

pub(crate) fn block_leaf_runtime(tag: Option<&str>) -> OutboundLeafRuntime {
    OutboundLeafRuntime {
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
        kernel_tag: tag.map(str::to_owned),
        #[cfg(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        ))]
        udp_policy_tag: tag.map(str::to_owned),
    }
}

impl ProtocolRegistry {
    fn claimed_outbound_entry(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<&RegisteredProtocolEntry> {
        self.entries
            .iter()
            .find(|entry| entry.tcp.claims_outbound_leaf(leaf))
    }

    fn required_outbound_entry(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<&RegisteredProtocolEntry, EngineError> {
        self.claimed_outbound_entry(leaf)
            .ok_or_else(unsupported_outbound_leaf)
    }

    fn claimed_runtime_and_entry<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<(OutboundLeafRuntime, Option<&RegisteredProtocolEntry>), EngineError> {
        let runtime = self.outbound_leaf_runtime(leaf)?;
        if matches!(runtime.tcp_path, TcpPathCategory::Block) {
            return Ok((runtime, None));
        }
        let entry = self.required_outbound_entry(leaf)?;
        Ok((runtime, Some(entry)))
    }

    /// Single dispatch point: the inventory claims a [`ResolvedLeafOutbound`]
    /// once and receives neutral runtime facts plus the compiled capabilities
    /// that own that leaf.
    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        let (runtime, entry) = self.claimed_runtime_and_entry(&leaf)?;
        Ok(ClaimedOutboundLeaf::new(leaf, runtime, entry))
    }

    /// Return neutral runtime facts for a resolved outbound leaf.
    ///
    /// Kernel-level `block` is handled here because no adapter owns it.
    /// Direct and proxy protocols are delegated to the adapter that claims the
    /// leaf, so runtime code does not match protocol variants.
    pub(crate) fn outbound_leaf_runtime(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<OutboundLeafRuntime, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf {
            return Ok(block_leaf_runtime(*tag));
        }

        let entry = self
            .claimed_outbound_entry(leaf)
            .ok_or_else(undescribed_outbound_leaf)?;
        entry
            .tcp
            .outbound_leaf_runtime(leaf)
            .ok_or_else(undescribed_outbound_leaf)
    }
}

fn unsupported_outbound_leaf() -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no compiled adapter handles this outbound leaf",
    ))
}

fn undescribed_outbound_leaf() -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no compiled adapter describes this outbound leaf",
    ))
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
fn missing_udp_relay_capability() -> crate::runtime::udp_dispatch::FlowFailure {
    crate::runtime::udp_dispatch::FlowFailure {
        stage: "find_outbound_leaf",
        error: EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "block outbound cannot provide a udp relay capability",
        )),
        upstream: None,
    }
}
