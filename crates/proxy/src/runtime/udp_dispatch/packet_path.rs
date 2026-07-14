use super::{FlowFailure, UdpDispatch};
use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::packet_path::PacketPathFlowBinding;
use crate::runtime::udp_flow::packet_path_chain::PacketPathStartRequest;

impl UdpDispatch {
    pub(crate) fn datagram_chain_flow_outbound(
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

    pub(crate) async fn send_packet_path_chain(
        &mut self,
        ctx: UdpAdapterContext<'_>,
        request: PacketPathStartRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.flow_state.send_packet_path_chain(ctx, request).await
    }
}
