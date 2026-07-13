use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use super::{
    BoundInbound, InboundAdapterContext, OutboundAdapterContext, OutboundLeafRuntime,
    UdpAdapterContext,
};
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
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

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

    /// Spawn the inbound accept loop for `inbound` into `listeners`.
    fn spawn_inbound(
        &self,
        _ctx: InboundAdapterContext<'_>,
        _inbound: InboundConfig,
        _bound: BoundInbound,
        _shutdown_rx: tokio::sync::watch::Receiver<bool>,
        _listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
    }
}

#[async_trait]
pub(crate) trait TcpOutboundCapability: Send + Sync {
    fn claims_outbound_leaf(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        None
    }

    async fn connect_tcp(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        Err(super::defaults::tcp_outbound_unsupported())
    }

    async fn apply_relay_hop(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = stream;
        Err(super::defaults::relay_hop_unsupported())
    }
}

#[async_trait]
pub(crate) trait UdpFlowCapability: Send + Sync {
    async fn start_udp_flow(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(super::defaults::udp_outbound_unsupported())
    }

    fn udp_relay_needs_two_streams(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    async fn start_udp_relay_two_stream(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        _chain: Vec<ResolvedLeafOutbound<'_>>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(super::defaults::udp_two_stream_relay_unsupported())
    }

    async fn start_udp_relay_final_hop(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        carrier: crate::transport::RelayCarrier,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let _ = carrier;
        Err(super::defaults::udp_relay_final_hop_unsupported())
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

#[async_trait]
pub(crate) trait UdpPacketPathCapability: Send + Sync {
    fn udp_packet_path_carrier_descriptor(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        None
    }

    async fn build_udp_packet_path(
        &self,
        _ctx: UdpAdapterContext<'_>,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        Err(super::defaults::packet_path_carrier_unsupported())
    }

    fn udp_datagram_source(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        None
    }
}
