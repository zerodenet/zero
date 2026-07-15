#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use std::iter;
use std::path::Path;

use zero_engine::{EngineError, ResolvedLeafOutbound, ResolvedOutbound};

use super::ProtocolInventory;
use crate::protocol_registry::{ClaimedOutboundLeaf, OutboundLeafRuntime};
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
use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

#[derive(Clone)]
pub(crate) struct ClaimedInventoryLeaf<'a> {
    claimed: ClaimedOutboundLeaf<'a>,
}

impl<'a> ClaimedInventoryLeaf<'a> {
    fn new(claimed: ClaimedOutboundLeaf<'a>) -> Self {
        Self { claimed }
    }

    pub(crate) fn runtime(&self) -> OutboundLeafRuntime {
        self.claimed.runtime.clone()
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
    pub(crate) fn into_claimed(self) -> ClaimedOutboundLeaf<'a> {
        self.claimed
    }

    pub(crate) fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    {
        self.claimed.prepare_tcp_connect(source_dir)
    }

    pub(crate) fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<(String, u16, Box<dyn PreparedTcpRelayOperation + 'a>), EngineError> {
        self.claimed.prepare_tcp_relay_hop(source_dir)
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
        self.claimed.prepare_udp_flow(source_dir)
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
        self.claimed.udp_relay_needs_two_streams(source_dir)
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
        self.claimed.prepare_udp_packet_path()
    }
}

pub(crate) enum ClaimedResolvedOutbound<'a> {
    Relay(ClaimedRelayChain<'a>),
    Single(ClaimedInventoryLeaf<'a>),
    Fallback(Vec<ClaimedInventoryLeaf<'a>>),
}

#[derive(Clone)]
pub(crate) struct ClaimedRelayChain<'a> {
    first: ClaimedInventoryLeaf<'a>,
    relay_hops: Vec<ClaimedInventoryLeaf<'a>>,
}

impl<'a> ClaimedRelayChain<'a> {
    pub(crate) fn new(
        first: ClaimedInventoryLeaf<'a>,
        relay_hops: Vec<ClaimedInventoryLeaf<'a>>,
    ) -> Self {
        Self { first, relay_hops }
    }

    pub(crate) fn first(&self) -> &ClaimedInventoryLeaf<'a> {
        &self.first
    }

    pub(crate) fn relay_hops(&self) -> &[ClaimedInventoryLeaf<'a>] {
        &self.relay_hops
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
    pub(crate) fn len(&self) -> usize {
        1 + self.relay_hops.len()
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
    pub(crate) fn final_hop(&self) -> &ClaimedInventoryLeaf<'a> {
        self.relay_hops
            .last()
            .expect("relay chain must have at least 2 hops")
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
    pub(crate) fn leaves(&self) -> impl Iterator<Item = &ClaimedInventoryLeaf<'a>> {
        iter::once(&self.first).chain(self.relay_hops.iter())
    }
}

impl ProtocolInventory {
    pub(crate) fn on_config_reloaded(&self) {
        self.registry.on_config_reloaded();
    }

    pub(super) fn claim_tcp_outbound<'a>(
        &self,
        resolved: &'a ResolvedOutbound<'a>,
    ) -> Result<ClaimedResolvedOutbound<'a>, TcpOutboundFailure> {
        match resolved {
            ResolvedOutbound::Single(candidate) => Ok(ClaimedResolvedOutbound::Single(
                self.claim_outbound_leaf(candidate.clone())
                    .map_err(map_tcp_outbound_leaf_runtime_failure)?,
            )),
            ResolvedOutbound::Relay { chain } => Ok(ClaimedResolvedOutbound::Relay(
                self.claim_tcp_relay_chain(chain.iter().cloned())?,
            )),
            ResolvedOutbound::Fallback { candidates } => {
                let mut claimed = Vec::with_capacity(candidates.len());
                let mut last_failure = None;

                for candidate in candidates.iter().cloned() {
                    match self.claim_outbound_leaf(candidate) {
                        Ok(candidate) => claimed.push(candidate),
                        Err(error) => {
                            last_failure = Some(map_tcp_outbound_leaf_runtime_failure(error))
                        }
                    }
                }

                if claimed.is_empty() {
                    Err(last_failure
                        .expect("validated fallback groups always have at least one candidate"))
                } else {
                    Ok(ClaimedResolvedOutbound::Fallback(claimed))
                }
            }
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
    pub(super) fn claim_udp_outbound<'a>(
        &self,
        resolved: &'a ResolvedOutbound<'a>,
    ) -> Result<ClaimedResolvedOutbound<'a>, FlowFailure> {
        match resolved {
            ResolvedOutbound::Single(candidate) => Ok(ClaimedResolvedOutbound::Single(
                self.claim_outbound_leaf(candidate.clone())
                    .map_err(map_udp_outbound_leaf_runtime_failure)?,
            )),
            ResolvedOutbound::Relay { chain } => Ok(ClaimedResolvedOutbound::Relay(
                self.claim_udp_relay_chain(chain.iter().cloned())?,
            )),
            ResolvedOutbound::Fallback { candidates } => {
                let mut claimed = Vec::with_capacity(candidates.len());
                let mut last_failure = None;

                for candidate in candidates.iter().cloned() {
                    match self.claim_outbound_leaf(candidate) {
                        Ok(candidate) => claimed.push(candidate),
                        Err(error) => {
                            last_failure = Some(map_udp_outbound_leaf_runtime_failure(error))
                        }
                    }
                }

                if claimed.is_empty() {
                    Err(last_failure
                        .expect("validated fallback groups always have at least one candidate"))
                } else {
                    Ok(ClaimedResolvedOutbound::Fallback(claimed))
                }
            }
        }
    }

    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedInventoryLeaf<'a>, EngineError> {
        let claimed = self.registry.claim_outbound_leaf(leaf)?;
        Ok(ClaimedInventoryLeaf::new(claimed))
    }
}

fn map_tcp_outbound_leaf_runtime_failure(error: EngineError) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "outbound_leaf_runtime",
        error,
        upstream_endpoint: None,
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
fn map_udp_outbound_leaf_runtime_failure(error: EngineError) -> FlowFailure {
    FlowFailure {
        stage: "outbound_leaf_runtime",
        error,
        upstream: None,
    }
}
