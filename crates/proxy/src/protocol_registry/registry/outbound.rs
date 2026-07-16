use std::path::Path;
use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{ProtocolRegistry, RegisteredProtocolEntry};
use crate::protocol_registry::{ClaimedTcpOutboundLeaf, OutboundLeafClaim, OutboundLeafRuntime};
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
        runtime: OutboundLeafRuntime,
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
    ) -> Self {
        Self {
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
            udp,
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
    pub(crate) fn prepare_udp_relay(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::relay::PreparedUdpRelayOperation<'a> + 'a>,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let capability = self
            .udp
            .capability
            .as_ref()
            .ok_or_else(missing_udp_relay_capability)?;
        capability.prepare_udp_relay(source_dir)
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

fn claim_outbound_hooks<'a>(
    entry: &RegisteredProtocolEntry,
    leaf: ResolvedLeafOutbound<'a>,
) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
    let Some(OutboundLeafClaim {
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
        udp,
        #[cfg(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        ))]
        packet_path,
    }) = entry.outbound.claim_outbound_leaf(leaf.clone())
    else {
        return Err(missing_claimed_outbound_leaf(entry.support.name(), &leaf));
    };
    let tcp = ClaimedTcpHooks {
        capability: Some(Arc::from(tcp) as Arc<dyn ClaimedTcpOutboundLeaf<'a> + 'a>),
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
    let udp = ClaimedUdpHooks {
        capability: udp.map(|claimed| Arc::from(claimed) as Arc<dyn ClaimedUdpFlowLeaf<'a> + 'a>),
        packet_path: packet_path
            .map(|claimed| Arc::from(claimed) as Arc<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>),
    };
    Ok(ClaimedOutboundLeaf::new(
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
        udp,
    ))
}

impl ProtocolRegistry {
    fn outbound_protocol_entry(&self, protocol: &str) -> Option<&RegisteredProtocolEntry> {
        self.entries
            .iter()
            .find(|entry| entry.support.name() == protocol)
    }

    /// Single dispatch point: the inventory claims a [`ResolvedLeafOutbound`]
    /// once and receives neutral runtime facts plus the compiled capabilities
    /// that own that leaf.
    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf.clone() {
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            let udp = ClaimedUdpHooks::default();
            return Ok(ClaimedOutboundLeaf::new(
                OutboundLeafRuntime::block(tag),
                ClaimedTcpHooks::default(),
                #[cfg(any(
                    feature = "socks5",
                    feature = "vless",
                    feature = "hysteria2",
                    feature = "shadowsocks",
                    feature = "trojan",
                    feature = "vmess",
                    feature = "mieru"
                ))]
                udp,
            ));
        }

        let protocol = leaf.protocol_name();
        let entry = self
            .outbound_protocol_entry(protocol)
            .ok_or_else(|| unsupported_outbound_leaf(protocol))?;
        claim_outbound_hooks(entry, leaf)
    }
}

fn unsupported_outbound_leaf(protocol: &str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("no compiled adapter claims outbound protocol `{protocol}`"),
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

fn missing_claimed_outbound_leaf(protocol: &str, leaf: &ResolvedLeafOutbound<'_>) -> EngineError {
    EngineError::Io(std::io::Error::other(format!(
        "{protocol} adapter owns outbound leaf `{}` but did not provide a claimed outbound leaf",
        leaf.protocol_name()
    )))
}
