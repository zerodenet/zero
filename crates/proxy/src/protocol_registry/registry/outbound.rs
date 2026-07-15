use std::path::{Path, PathBuf};
use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{ProtocolRegistry, RegisteredProtocolEntry};
use crate::protocol_registry::{ClaimedTcpOutboundLeaf, OutboundLeafRuntime};
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

type TcpConnectPrepareHook<'a> = dyn Fn(
        Option<PathBuf>,
    )
        -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    + Send
    + Sync
    + 'a;
type TcpRelayPrepareHook<'a> = dyn Fn(Option<PathBuf>) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError>
    + Send
    + Sync
    + 'a;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
type UdpFlowPrepareHook<'a> = dyn Fn(
        Option<PathBuf>,
    )
        -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    + Send
    + Sync
    + 'a;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
type UdpRelayNeedsTwoStreamsHook<'a> = dyn Fn(Option<PathBuf>) -> bool + Send + Sync + 'a;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
type UdpRelayFinalHopPrepareHook<'a> = dyn Fn(
        RelayCarrier,
        Option<PathBuf>,
    )
        -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    + Send
    + Sync
    + 'a;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
type UdpRelayTwoStreamPrepareHook<'a> = dyn Fn(
        RelayCarrier,
        RelayCarrier,
        Option<PathBuf>,
    )
        -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    + Send
    + Sync
    + 'a;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
type UdpPacketPathPrepareHook<'a> = dyn Fn() -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    > + Send
    + Sync
    + 'a;

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
    flow: Option<Arc<UdpFlowPrepareHook<'a>>>,
    relay_needs_two_streams: Option<Arc<UdpRelayNeedsTwoStreamsHook<'a>>>,
    relay_final_hop: Option<Arc<UdpRelayFinalHopPrepareHook<'a>>>,
    relay_two_stream: Option<Arc<UdpRelayTwoStreamPrepareHook<'a>>>,
    packet_path: Option<Arc<UdpPacketPathPrepareHook<'a>>>,
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
        leaf: ResolvedLeafOutbound<'a>,
        runtime: OutboundLeafRuntime,
        entry: Option<&RegisteredProtocolEntry>,
    ) -> Self {
        Self {
            runtime,
            tcp: build_tcp_hooks(entry, leaf.clone()),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp: build_udp_hooks(entry, leaf),
        }
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
        self.udp.flow.is_some()
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
        let prepare = self
            .udp
            .flow
            .as_ref()
            .expect("non-block udp leaf must expose a udp-flow capability");
        prepare(source_dir.map(Path::to_path_buf))
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
            .relay_needs_two_streams
            .as_ref()
            .expect("udp relay leaf must expose a udp-flow capability")(
            source_dir.map(Path::to_path_buf),
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
    pub(crate) fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        let prepare = self
            .udp
            .relay_final_hop
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?;
        prepare(carrier, source_dir.map(Path::to_path_buf))
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
        let prepare = self
            .udp
            .relay_two_stream
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?;
        prepare(post_carrier, get_carrier, source_dir.map(Path::to_path_buf))
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
        let prepare = self.udp.packet_path.as_ref()?;
        prepare()
    }
}

struct HookClaimedTcpLeaf<'a> {
    connect: Arc<TcpConnectPrepareHook<'a>>,
    relay_hop: Arc<TcpRelayPrepareHook<'a>>,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for HookClaimedTcpLeaf<'a> {
    fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    {
        (self.connect)(source_dir.map(Path::to_path_buf))
    }

    fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        (self.relay_hop)(source_dir.map(Path::to_path_buf))
    }
}

fn build_tcp_hooks<'a>(
    entry: Option<&RegisteredProtocolEntry>,
    leaf: ResolvedLeafOutbound<'a>,
) -> ClaimedTcpHooks<'a> {
    let Some(entry) = entry else {
        return ClaimedTcpHooks::default();
    };
    if let Some(claimed) = entry.tcp.claim_tcp_outbound_leaf(leaf.clone()) {
        return ClaimedTcpHooks {
            capability: Some(Arc::from(claimed)),
        };
    }
    let tcp_connect = {
        let tcp = entry.tcp.clone();
        let leaf = leaf.clone();
        Arc::new(move |source_dir: Option<PathBuf>| {
            tcp.prepare_tcp_connect(leaf.clone(), source_dir.as_deref())
        }) as Arc<TcpConnectPrepareHook<'a>>
    };
    let tcp_relay_hop = {
        let tcp = entry.tcp.clone();
        let leaf = leaf.clone();
        Arc::new(move |source_dir: Option<PathBuf>| {
            tcp.prepare_tcp_relay_hop(leaf.clone(), source_dir.as_deref())
        }) as Arc<TcpRelayPrepareHook<'a>>
    };
    ClaimedTcpHooks {
        capability: Some(Arc::new(HookClaimedTcpLeaf {
            connect: tcp_connect,
            relay_hop: tcp_relay_hop,
        })),
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
) -> ClaimedUdpHooks<'a> {
    let Some(entry) = entry else {
        return ClaimedUdpHooks::default();
    };
    let Some(udp) = entry.udp.clone() else {
        return ClaimedUdpHooks {
            packet_path: entry.packet_path.as_ref().map(|packet_path| {
                let packet_path = packet_path.clone();
                let leaf = leaf.clone();
                Arc::new(move || packet_path.prepare_udp_packet_path(leaf.clone()))
                    as Arc<UdpPacketPathPrepareHook<'a>>
            }),
            ..ClaimedUdpHooks::default()
        };
    };

    let flow = {
        let udp = udp.clone();
        let leaf = leaf.clone();
        Arc::new(move |source_dir: Option<PathBuf>| {
            udp.prepare_udp_flow(leaf.clone(), source_dir.as_deref())
        }) as Arc<UdpFlowPrepareHook<'a>>
    };
    let relay_needs_two_streams = {
        let udp = udp.clone();
        let leaf = leaf.clone();
        Arc::new(move |source_dir: Option<PathBuf>| {
            udp.udp_relay_needs_two_streams(&leaf, source_dir.as_deref())
        }) as Arc<UdpRelayNeedsTwoStreamsHook<'a>>
    };
    let relay_final_hop = {
        let udp = udp.clone();
        let leaf = leaf.clone();
        Arc::new(move |carrier, source_dir: Option<PathBuf>| {
            udp.prepare_owned_udp_relay_final_hop(carrier, leaf.clone(), source_dir.as_deref())
        }) as Arc<UdpRelayFinalHopPrepareHook<'a>>
    };
    let relay_two_stream = {
        let udp = udp.clone();
        let leaf = leaf.clone();
        Arc::new(
            move |post_carrier, get_carrier, source_dir: Option<PathBuf>| {
                udp.prepare_owned_udp_relay_two_stream(
                    post_carrier,
                    get_carrier,
                    leaf.clone(),
                    source_dir.as_deref(),
                )
            },
        ) as Arc<UdpRelayTwoStreamPrepareHook<'a>>
    };
    let packet_path = entry.packet_path.as_ref().map(|packet_path| {
        let packet_path = packet_path.clone();
        let leaf = leaf.clone();
        Arc::new(move || packet_path.prepare_udp_packet_path(leaf.clone()))
            as Arc<UdpPacketPathPrepareHook<'a>>
    });

    ClaimedUdpHooks {
        flow: Some(flow),
        relay_needs_two_streams: Some(relay_needs_two_streams),
        relay_final_hop: Some(relay_final_hop),
        relay_two_stream: Some(relay_two_stream),
        packet_path,
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
