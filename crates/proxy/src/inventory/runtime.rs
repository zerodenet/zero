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

use zero_engine::{EngineError, ResolvedLeafOutbound};

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

pub(crate) struct ClaimedInventoryLeaf<'a> {
    leaf: &'a ResolvedLeafOutbound<'a>,
    claimed: ClaimedOutboundLeaf<'a>,
}

impl<'a> ClaimedInventoryLeaf<'a> {
    fn new(leaf: &'a ResolvedLeafOutbound<'a>, claimed: ClaimedOutboundLeaf<'a>) -> Self {
        Self { leaf, claimed }
    }

    pub(crate) fn runtime(&self) -> OutboundLeafRuntime<'a> {
        self.claimed.runtime
    }

    #[cfg(test)]
    pub(crate) fn into_claimed(self) -> ClaimedOutboundLeaf<'a> {
        self.claimed
    }

    pub(crate) fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, crate::transport::TcpOutboundFailure>
    {
        self.claimed.prepare_tcp_connect(self.leaf, source_dir)
    }

    pub(crate) fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<(&'a str, u16, Box<dyn PreparedTcpRelayOperation + 'a>), EngineError> {
        self.claimed.prepare_tcp_relay_hop(self.leaf, source_dir)
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
        self.claimed.prepare_udp_flow(self.leaf, source_dir)
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
        self.claimed.prepare_udp_packet_path(self.leaf)
    }
}

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

    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedInventoryLeaf<'a>, EngineError> {
        let claimed = self.registry.claim_outbound_leaf(leaf)?;
        Ok(ClaimedInventoryLeaf::new(leaf, claimed))
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
    pub(crate) fn claim_owned_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        let claimed = self.registry.claim_outbound_leaf(&leaf)?;
        Ok(claimed)
    }
}
