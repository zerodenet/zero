use zero_engine::ResolvedLeafOutbound;

use super::ProtocolUdpState;
use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::protocol_runtime::udp::{FlowFailure, UdpDatagramSource, UdpPacketPathCarrier};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) fn datagram_chain_flow_outbound(
        &self,
        datagram: UdpDatagramSource<'_>,
        packet_path_carrier: Option<UdpPacketPathCarrier>,
    ) -> UdpFlowOutbound {
        UdpFlowOutbound::Shadowsocks {
            tag: datagram.tag.to_owned(),
            server: datagram.server.to_owned(),
            port: datagram.port,
            password: datagram.password.to_owned(),
            cipher: datagram.cipher.to_owned(),
            packet_path_carrier,
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
