use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use super::{BoundInbound, OutboundLeafRuntime};
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
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_flow::managed::model::ManagedDatagramFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedStreamHandlerPair;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::transport::TcpOutboundFailure;

pub(crate) trait ClaimedTcpOutboundLeaf<'a>: Send + Sync {
    fn runtime(&self) -> OutboundLeafRuntime;

    fn prepare_tcp_connect(
        &self,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure>;

    fn prepare_tcp_relay_hop(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        Err(super::defaults::relay_hop_unsupported())
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
pub(crate) trait ClaimedUdpFlowLeaf<'a>: Send + Sync {
    fn prepare_udp_flow(
        &self,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>;

    fn udp_relay_needs_two_streams(&self, _source_dir: Option<&std::path::Path>) -> bool {
        false
    }

    fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: crate::transport::RelayCarrier,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        let _ = carrier;
        Err(super::defaults::udp_relay_final_hop_unsupported())
    }

    fn prepare_owned_udp_relay_two_stream(
        &self,
        post_carrier: crate::transport::RelayCarrier,
        get_carrier: crate::transport::RelayCarrier,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, crate::runtime::udp_dispatch::FlowFailure>
    {
        let _ = (post_carrier, get_carrier);
        Err(super::defaults::udp_two_stream_relay_unsupported())
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
pub(crate) trait ClaimedUdpPacketPathLeaf<'a>: Send + Sync {
    fn prepare_udp_packet_path(
        &self,
    ) -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    >;
}

pub(crate) struct OutboundLeafClaim<'a> {
    pub(crate) runtime: OutboundLeafRuntime,
    pub(crate) tcp: Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) udp: Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) packet_path: Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>>,
}

pub(crate) trait ProtocolSupportCapability: ProtocolMetadata + Send + Sync {
    fn name(&self) -> &'static str;
    fn feature_name(&self) -> &'static str;
    fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool;
    fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool;
    fn has_inbound(&self) -> bool;
    fn has_outbound(&self) -> bool;

    fn on_config_reloaded(&self) {}
}

#[async_trait]
pub(crate) trait InboundListenerCapability: Send + Sync {
    /// Bind the listener socket eagerly so port-in-use errors surface before
    /// the proxy announces "started".
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        super::defaults::bind_tcp_inbound(inbound).await
    }

    /// Validate protocol-local listener state and prepare a runtime-executed
    /// listener operation.
    fn prepare_inbound_listener(
        &self,
        _inbound: InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this adapter does not provide an inbound listener",
        )))
    }
}

pub(crate) trait TcpOutboundCapability: Send + Sync {
    fn claim_tcp_outbound_leaf<'a>(
        &self,
        _leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        None
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
pub(crate) trait UdpFlowCapability: Send + Sync {
    fn claim_udp_flow_leaf<'a>(
        &self,
        _leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        None
    }
}

#[cfg(feature = "socks5")]
pub(crate) trait UpstreamUdpHandlerProvider: Send + Sync {
    fn upstream_association_handler(&self) -> Box<dyn UpstreamAssociationHandler>;
}

#[cfg(any(
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) trait ManagedUdpHandlerProvider: Send + Sync {
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        None
    }

    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        None
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
pub(crate) trait UdpPacketPathCapability: Send + Sync {
    fn claim_udp_packet_path_leaf<'a>(
        &self,
        _leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>> {
        None
    }
}
