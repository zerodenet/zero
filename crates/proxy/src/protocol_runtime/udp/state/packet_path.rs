use zero_engine::ResolvedLeafOutbound;

use super::ProtocolUdpState;
use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::protocol_runtime::udp::{FlowFailure, PacketPathFlowBinding};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) fn datagram_chain_flow_outbound(
        &self,
        flow_binding: PacketPathFlowBinding<'_>,
    ) -> UdpFlowOutbound {
        let (datagram, flow_snapshot, packet_path_carrier) = flow_binding.into_parts();
        let descriptor = datagram.descriptor();
        let tag = descriptor.tag.to_owned();
        let server = descriptor.server.to_owned();
        let port = descriptor.port;
        let protocol = flow_snapshot
            .with_packet_path_carrier(packet_path_carrier)
            .into_protocol_snapshot();

        UdpFlowOutbound::Datagram {
            tag,
            server,
            port,
            protocol,
        }
    }

    pub(crate) async fn send_packet_path_chain(
        &mut self,
        context: UdpFlowContext<'_>,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        self.packet_path
            .send(context, proxy, carrier_leaf, datagram_leaf, packet)
            .await
    }
}
