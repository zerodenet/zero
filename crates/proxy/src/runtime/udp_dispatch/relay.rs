use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::packet_path::PacketPathFlowBinding;
use crate::runtime::udp_flow::packet_path_chain::PacketPathStartRequest;

pub(crate) enum PreparedUdpRelayChain<'a> {
    PacketPath {
        flow_binding: PacketPathFlowBinding,
        request: Box<PacketPathStartRequest<'a>>,
    },
    Operation(Box<dyn PreparedUdpFlowOperation + 'a>),
}

impl PreparedUdpRelayChain<'_> {
    pub(crate) async fn execute(
        self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        match self {
            Self::PacketPath {
                flow_binding,
                request,
            } => {
                let sent = dispatch.send_packet_path_chain(ctx, *request).await?;
                Ok(FlowStartResult::Flow {
                    outbound: Box::new(UdpDispatch::datagram_chain_flow_outbound(flow_binding)),
                    tx_bytes: sent as u64,
                })
            }
            Self::Operation(operation) => operation.execute(dispatch, ctx, session, payload).await,
        }
    }
}
