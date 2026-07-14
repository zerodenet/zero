use super::super::ProtocolInventory;
use crate::protocol_registry::UdpPacketPathCapability;
use crate::runtime::udp_flow::packet_path::{PacketPathFlowBinding, UdpPacketRef};
use crate::runtime::udp_flow::packet_path_chain::{
    PacketPathCarrierRequest, PacketPathStartRequest,
};

impl ProtocolInventory {
    /// Prepare the packet-path carrier/datagram pair and lazy carrier builder
    /// for a two-hop UDP relay chain.
    pub(crate) fn prepare_udp_packet_path_pair<'a>(
        &self,
        session_id: u64,
        carrier_leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
        datagram_leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
        packet: UdpPacketRef<'a>,
    ) -> Option<(PacketPathFlowBinding, PacketPathStartRequest<'a>)> {
        let carrier_adapter = self.registry.find_udp_packet_path_leaf(carrier_leaf).ok()?;
        let datagram_adapter = self
            .registry
            .find_udp_packet_path_leaf(datagram_leaf)
            .ok()?;

        let carrier_operation = UdpPacketPathCapability::prepare_udp_packet_path(
            carrier_adapter.as_ref(),
            carrier_leaf,
        )?;
        let datagram_operation = UdpPacketPathCapability::prepare_udp_packet_path(
            datagram_adapter.as_ref(),
            datagram_leaf,
        )?;

        let carrier_desc = carrier_operation.carrier_descriptor()?;
        let datagram = datagram_operation.datagram_source()?;
        let flow_binding = PacketPathFlowBinding::new(datagram.clone(), &carrier_desc);

        Some((
            flow_binding,
            PacketPathStartRequest {
                session_id,
                carrier: PacketPathCarrierRequest {
                    descriptor: carrier_desc,
                    build_operation: carrier_operation,
                },
                datagram,
                packet,
            },
        ))
    }
}
