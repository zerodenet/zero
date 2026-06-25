use std::fmt;

use async_trait::async_trait;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::defaults;
use super::UdpAdapterContext;
/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
#[async_trait]
pub(crate) trait ProtocolAdapter: Send + Sync + fmt::Debug {
    /// If this leaf can serve as a UDP packet-path carrier (relay-chain first
    /// hop that provides a raw send/recv channel), return its identity
    /// descriptor (cache key + endpoint). Cheap; used for cache lookup before
    /// [`Self::build_udp_packet_path`] dials.
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        None
    }

    /// Owned snapshot of the carrier for flow status/result reporting.
    ///
    /// Only carrier-capable adapters override this. The runtime uses it when a
    /// relay chain caches a packet-path carrier and needs to keep a stable
    /// owned representation in `UdpFlowOutbound`.
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        None
    }

    /// Build the concrete packet-path carrier for this leaf (dial + establish).
    /// Only called on a cache miss. Defaults to "not supported".
    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        _ctx: UdpAdapterContext<'_>,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError>
    {
        Err(defaults::packet_path_carrier_unsupported())
    }

    /// If this leaf can be a UDP packet-path datagram source (relay-chain final
    /// hop that encodes its datagram through a carrier), return its params.
    /// `None` for protocols that cannot serve this role.
    #[cfg(feature = "shadowsocks")]
    fn udp_datagram_source<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::protocol_runtime::udp::UdpDatagramSource<'a>> {
        None
    }
}
