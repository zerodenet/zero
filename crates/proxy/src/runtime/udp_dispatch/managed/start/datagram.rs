#[cfg(feature = "managed-datagram-runtime")]
use crate::runtime::udp_dispatch::managed::model::{ManagedDatagramStart, ManagedUdpSend};
#[cfg(feature = "managed-datagram-runtime")]
use crate::runtime::udp_dispatch::operation::ManagedDatagramStartPlan;
#[cfg(any(
    feature = "upstream-association-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_dispatch::FlowStartResult;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
#[cfg(feature = "managed-datagram-runtime")]
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
#[cfg(any(
    feature = "upstream-association-runtime",
    feature = "managed-datagram-runtime"
))]
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;

impl UdpDispatch {
    #[cfg(feature = "managed-datagram-runtime")]
    async fn start_tracked_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let resume = request.resume.clone();
        let tag = request.tag.to_string();
        let server = request.server.to_string();
        let port = request.port;
        let sent = self.send_managed_udp(request).await?;
        let managed = self.register_managed_flow(resume);
        let outbound = UdpFlowOutbound::Datagram {
            tag,
            server,
            port,
            managed,
        };
        Ok(FlowStartResult::Flow {
            outbound: Box::new(outbound),
            tx_bytes: sent as u64,
        })
    }

    #[cfg(feature = "managed-datagram-runtime")]
    pub(crate) async fn start_transport_managed_datagram<T>(
        &mut self,
        services: Option<crate::protocol_registry::UdpRuntimeServices>,
        session: &zero_core::Session,
        payload: &[u8],
        plan: ManagedDatagramStartPlan<T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        let ManagedDatagramStartPlan {
            tag,
            server,
            port,
            resume,
        } = plan;
        self.start_tracked_managed_datagram(ManagedDatagramStart {
            services,
            tag: &tag,
            session,
            server: &server,
            port,
            resume,
            payload,
        })
        .await
    }

    #[cfg(feature = "managed-datagram-runtime")]
    pub(crate) async fn start_tracked_managed_datagram<T>(
        &mut self,
        request: ManagedDatagramStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        self.start_tracked_managed_udp(ManagedUdpSend {
            services: request.services,
            tag: request.tag,
            session: request.session,
            #[cfg(feature = "managed-stream-runtime")]
            carrier: None,
            #[cfg(feature = "managed-stream-runtime")]
            tls_server_name: None,
            server: request.server,
            port: request.port,
            resume: ManagedUdpFlowResume::new(request.resume),
            payload: request.payload,
            kind: ManagedUdpFlowKind::Datagram,
        })
        .await
    }
}
