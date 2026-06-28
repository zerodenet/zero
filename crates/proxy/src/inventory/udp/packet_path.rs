use zero_engine::EngineError;

use super::super::ProtocolInventory;
use crate::protocol_registry::{UdpAdapterContext, UdpPacketPathCapability};
use crate::runtime::Proxy;

impl ProtocolInventory {
    /// Return packet-path datagram params and carrier snapshot when the two
    /// relay-chain leaves form a supported packet-path pair.
    pub(crate) fn udp_packet_path_pair(
        &self,
        carrier_leaf: &zero_engine::ResolvedLeafOutbound<'_>,
        datagram_leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathFlowBinding> {
        let carrier_adapter = self.registry.find_outbound_leaf(carrier_leaf).ok()?;
        let datagram_adapter = self.registry.find_outbound_leaf(datagram_leaf).ok()?;

        let carrier_desc = UdpPacketPathCapability::udp_packet_path_carrier_descriptor(
            carrier_adapter.as_ref(),
            carrier_leaf,
        )?;
        let datagram =
            UdpPacketPathCapability::udp_datagram_source(datagram_adapter.as_ref(), datagram_leaf)?;
        Some(
            crate::runtime::udp_flow::packet_path::PacketPathFlowBinding::new(
                datagram,
                &carrier_desc,
            ),
        )
    }

    /// Resolve packet-path entry construction params through the carrier and
    /// datagram adapters.
    pub(crate) fn resolve_udp_packet_path_candidate(
        &self,
        carrier_leaf: &zero_engine::ResolvedLeafOutbound<'_>,
        datagram_leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<
        (
            crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor,
            crate::runtime::udp_flow::packet_path::UdpDatagramSource,
        ),
        EngineError,
    > {
        let carrier_adapter = self.registry.find_outbound_leaf(carrier_leaf)?;
        let datagram_adapter = self.registry.find_outbound_leaf(datagram_leaf)?;
        let carrier_desc = UdpPacketPathCapability::udp_packet_path_carrier_descriptor(
            carrier_adapter.as_ref(),
            carrier_leaf,
        )
        .ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "outbound does not support UDP packet-path carrier role",
            ))
        })?;
        let datagram =
            UdpPacketPathCapability::udp_datagram_source(datagram_adapter.as_ref(), datagram_leaf)
                .ok_or_else(|| {
                    EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "outbound does not support UDP packet-path datagram role",
                    ))
                })?;
        Ok((carrier_desc, datagram))
    }

    /// Build the concrete packet-path carrier through the carrier adapter.
    pub(crate) async fn build_udp_packet_path_carrier(
        &self,
        proxy: &Proxy,
        carrier_leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let carrier_adapter = self.registry.find_outbound_leaf(carrier_leaf)?;
        UdpPacketPathCapability::build_udp_packet_path(
            carrier_adapter.as_ref(),
            UdpAdapterContext::new(proxy),
            carrier_leaf,
        )
        .await
    }
}
