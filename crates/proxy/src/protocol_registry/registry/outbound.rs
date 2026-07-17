use std::path::Path;
use std::sync::Arc;

use zero_config::RuntimeConfig;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{ProtocolRegistry, RegisteredProtocolEntry};
use crate::protocol_registry::{ClaimedTcpOutboundLeaf, OutboundLeafClaim, OutboundLeafRuntime};
#[cfg(feature = "udp-runtime")]
use crate::protocol_registry::{ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "udp-runtime")]
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
#[derive(Clone, Default)]
struct ClaimedTcpHooks<'a> {
    capability: Option<Arc<dyn ClaimedTcpOutboundLeaf<'a> + 'a>>,
}

#[cfg(feature = "udp-runtime")]
#[derive(Clone, Default)]
struct ClaimedUdpHooks<'a> {
    capability: Option<Arc<dyn ClaimedUdpFlowLeaf<'a> + 'a>>,
    packet_path: Option<Arc<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>>,
}

#[derive(Clone)]
pub(crate) struct ClaimedOutboundLeaf<'a> {
    pub(crate) runtime: OutboundLeafRuntime,
    tcp: ClaimedTcpHooks<'a>,
    #[cfg(feature = "udp-runtime")]
    udp: ClaimedUdpHooks<'a>,
}

impl<'a> ClaimedOutboundLeaf<'a> {
    fn new(
        runtime: OutboundLeafRuntime,
        tcp: ClaimedTcpHooks<'a>,
        #[cfg(feature = "udp-runtime")] udp: ClaimedUdpHooks<'a>,
    ) -> Self {
        Self {
            runtime,
            tcp,
            #[cfg(feature = "udp-runtime")]
            udp,
        }
    }

    #[cfg(test)]
    pub(crate) fn has_tcp_capability(&self) -> bool {
        self.tcp.capability.is_some()
    }

    #[cfg(feature = "udp-runtime")]
    #[cfg(test)]
    pub(crate) fn has_udp_flow_capability(&self) -> bool {
        self.udp.capability.is_some()
    }

    #[cfg(feature = "udp-runtime")]
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

    #[cfg(feature = "udp-runtime")]
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

    #[cfg(feature = "udp-runtime")]
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

    #[cfg(feature = "udp-runtime")]
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
    protocol: Option<&'a zero_config::OutboundProtocolConfig>,
    leaf: ResolvedLeafOutbound<'a>,
) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
    let Some(OutboundLeafClaim {
        runtime,
        tcp,
        #[cfg(feature = "udp-runtime")]
        udp,
        #[cfg(feature = "udp-runtime")]
        packet_path,
    }) = entry.outbound.claim_outbound_leaf(protocol, leaf.clone())
    else {
        return Err(missing_claimed_outbound_leaf(entry.support.name(), &leaf));
    };
    let tcp = ClaimedTcpHooks {
        capability: Some(Arc::from(tcp) as Arc<dyn ClaimedTcpOutboundLeaf<'a> + 'a>),
    };
    #[cfg(feature = "udp-runtime")]
    let udp = ClaimedUdpHooks {
        capability: udp.map(|claimed| Arc::from(claimed) as Arc<dyn ClaimedUdpFlowLeaf<'a> + 'a>),
        packet_path: packet_path
            .map(|claimed| Arc::from(claimed) as Arc<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>),
    };
    Ok(ClaimedOutboundLeaf::new(
        runtime,
        tcp,
        #[cfg(feature = "udp-runtime")]
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
        config: &'a RuntimeConfig,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf.clone() {
            #[cfg(feature = "udp-runtime")]
            let udp = ClaimedUdpHooks::default();
            return Ok(ClaimedOutboundLeaf::new(
                OutboundLeafRuntime::block(tag),
                ClaimedTcpHooks::default(),
                #[cfg(feature = "udp-runtime")]
                udp,
            ));
        }

        let protocol = leaf.protocol_name();
        let entry = self
            .outbound_protocol_entry(protocol)
            .ok_or_else(|| unsupported_outbound_leaf(protocol))?;
        if matches!(leaf, ResolvedLeafOutbound::Direct { .. }) {
            return claim_outbound_hooks(entry, None, leaf);
        }
        let outbound_index = leaf.outbound_index().ok_or_else(|| {
            EngineError::Io(std::io::Error::other(
                "configured proxy leaf is missing its outbound index",
            ))
        })?;
        let outbound = config.outbounds.get(outbound_index).ok_or_else(|| {
            EngineError::Io(std::io::Error::other(format!(
                "resolved outbound index {outbound_index} is outside the active config",
            )))
        })?;
        if outbound.protocol.protocol_name() != protocol {
            return Err(EngineError::Io(std::io::Error::other(format!(
                "resolved outbound protocol `{protocol}` does not match active config protocol `{}`",
                outbound.protocol.protocol_name()
            ))));
        }
        claim_outbound_hooks(entry, Some(&outbound.protocol), leaf)
    }
}

fn unsupported_outbound_leaf(protocol: &str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("no compiled adapter claims outbound protocol `{protocol}`"),
    ))
}

#[cfg(feature = "udp-runtime")]

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
