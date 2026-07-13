use super::{FlowFailure, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::packet_path::PacketPathFlowBinding;
use crate::runtime::udp_flow::packet_path_chain::PacketPathStartRequest;
use crate::runtime::Proxy;

impl UdpDispatch {
    pub(super) fn datagram_chain_flow_outbound(
        flow_binding: PacketPathFlowBinding,
    ) -> UdpFlowOutbound {
        let (datagram, flow_snapshot) = flow_binding.into_parts();
        let descriptor = datagram.descriptor();
        let tag = descriptor.tag.to_owned();
        let server = descriptor.server.to_owned();
        let port = descriptor.port;

        UdpFlowOutbound::PacketPathDatagram {
            tag,
            server,
            port,
            snapshot: flow_snapshot,
        }
    }

    pub(super) async fn send_packet_path_chain(
        &mut self,
        proxy: &Proxy,
        request: PacketPathStartRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.flow_state.send_packet_path_chain(proxy, request).await
    }
}
