#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_dispatch::managed::model::ManagedUdpSend;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;

impl UdpDispatch {
    #[cfg(any(
        feature = "managed-stream-runtime",
        feature = "managed-datagram-runtime"
    ))]
    pub(in crate::runtime::udp_dispatch::managed) async fn send_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.start_managed_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            services: request.services,
            kind: request.kind,
            session: request.session,
            #[cfg(feature = "managed-stream-runtime")]
            carrier: request.carrier,
            #[cfg(feature = "managed-stream-runtime")]
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
    }
}
