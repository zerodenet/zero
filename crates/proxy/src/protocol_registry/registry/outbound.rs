use std::path::Path;
use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{ProtocolRegistry, RegisteredProtocolEntry};
use crate::protocol_registry::{ClaimedTcpOutboundLeaf, OutboundLeafRuntime};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::protocol_registry::{ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf};
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

#[derive(Clone, Default)]
struct ClaimedTcpHooks<'a> {
    capability: Option<Arc<dyn ClaimedTcpOutboundLeaf<'a> + 'a>>,
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
#[derive(Clone, Default)]
struct ClaimedUdpHooks<'a> {
    capability: Option<Arc<dyn ClaimedUdpFlowLeaf<'a> + 'a>>,
    packet_path: Option<Arc<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>>,
}

#[derive(Clone)]
pub(crate) struct ClaimedOutboundLeaf<'a> {
    pub(crate) runtime: OutboundLeafRuntime,
    tcp: ClaimedTcpHooks<'a>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    udp: ClaimedUdpHooks<'a>,
}

impl<'a> ClaimedOutboundLeaf<'a> {
    fn new(
        _leaf: ResolvedLeafOutbound<'a>,
        runtime: OutboundLeafRuntime,
        tcp: ClaimedTcpHooks<'a>,
        _entry: Option<&RegisteredProtocolEntry>,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            runtime,
            tcp,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp: build_udp_hooks(_entry, _leaf)?,
        })
    }

    #[cfg(test)]
    pub(crate) fn has_tcp_capability(&self) -> bool {
        self.tcp.capability.is_some()
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
    #[cfg(test)]
    pub(crate) fn has_udp_flow_capability(&self) -> bool {
        self.udp.capability.is_some()
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
    #[cfg(test)]
    pub(crate) fn has_udp_packet_path_capability(&self) -> bool {
        self.udp.packet_path.is_some()
    }

    pub(crate) fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    {
        let capability = self
            .tcp
            .capability
            .as_ref()
            .expect("non-block tcp leaf must expose a tcp capability");
        capability.prepare_tcp_connect(source_dir)
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
        let capability = self
            .tcp
            .capability
            .as_ref()
            .expect("tcp relay hop must expose a tcp capability");
        let operation = capability.prepare_tcp_relay_hop(source_dir)?;
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
        let capability = self
            .udp
            .capability
            .as_ref()
            .expect("non-block udp leaf must expose a udp-flow capability");
        capability.prepare_udp_flow(source_dir)
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
            .capability
            .as_ref()
            .expect("udp relay leaf must expose a udp-flow capability")
            .udp_relay_needs_two_streams(source_dir)
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
        let capability = self
            .udp
            .capability
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?;
        capability.prepare_owned_udp_relay_final_hop(carrier, source_dir)
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
        let capability = self
            .udp
            .capability
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?;
        capability.prepare_owned_udp_relay_two_stream(post_carrier, get_carrier, source_dir)
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
        let capability = self.udp.packet_path.as_ref()?;
        capability.prepare_udp_packet_path()
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
fn build_udp_hooks<'a>(
    entry: Option<&RegisteredProtocolEntry>,
    leaf: ResolvedLeafOutbound<'a>,
) -> Result<ClaimedUdpHooks<'a>, EngineError> {
    let Some(entry) = entry else {
        return Ok(ClaimedUdpHooks::default());
    };
    let packet_path = entry
        .packet_path
        .as_ref()
        .and_then(|packet_path| packet_path.claim_udp_packet_path_leaf(leaf.clone()))
        .map(|claimed| Arc::from(claimed) as Arc<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>);
    let Some(udp) = entry.udp.clone() else {
        return Ok(ClaimedUdpHooks {
            packet_path,
            ..ClaimedUdpHooks::default()
        });
    };
    if let Some(claimed) = udp.claim_udp_flow_leaf(leaf.clone()) {
        return Ok(ClaimedUdpHooks {
            capability: Some(Arc::from(claimed)),
            packet_path,
        });
    }
    Err(missing_claimed_udp_leaf(entry.support.name(), &leaf))
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
    fn claimed_tcp_entry<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<
        (
            &RegisteredProtocolEntry,
            Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>,
        ),
        EngineError,
    > {
        for entry in &self.entries {
            if let Some(claimed) = entry.tcp.claim_tcp_outbound_leaf(leaf.clone()) {
                return Ok((entry, claimed));
            }
        }
        Err(unsupported_outbound_leaf())
    }

    /// Single dispatch point: the inventory claims a [`ResolvedLeafOutbound`]
    /// once and receives neutral runtime facts plus the compiled capabilities
    /// that own that leaf.
    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf.clone() {
            return ClaimedOutboundLeaf::new(
                leaf,
                block_leaf_runtime(tag),
                ClaimedTcpHooks::default(),
                None,
            );
        }

        let (entry, claimed_tcp) = self.claimed_tcp_entry(leaf.clone())?;
        let runtime = claimed_tcp.runtime();
        let tcp = ClaimedTcpHooks {
            capability: Some(Arc::from(claimed_tcp)),
        };
        ClaimedOutboundLeaf::new(leaf, runtime, tcp, Some(entry))
    }
}

fn unsupported_outbound_leaf() -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no compiled adapter handles this outbound leaf",
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn missing_claimed_udp_leaf(protocol: &str, leaf: &ResolvedLeafOutbound<'_>) -> EngineError {
    EngineError::Io(std::io::Error::other(format!(
        "{protocol} adapter claims outbound leaf `{}` but did not provide a claimed UDP leaf",
        leaf.protocol_name()
    )))
}
