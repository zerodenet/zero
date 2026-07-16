use std::future::Future;
use std::pin::Pin;

use zero_core::Session;

use super::contract::PreparedUdpFlowOperation;
use crate::protocol_registry::{UdpAdapterContext, UdpRuntimeServices};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};

pub(crate) struct ManagedDatagramUdpOperation<T> {
    pub(crate) plan: zero_transport::managed_udp::ManagedDatagramStartPlan<T>,
    pub(crate) needs_proxy: bool,
}

impl<T> PreparedUdpFlowOperation for ManagedDatagramUdpOperation<T>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
{
    fn execute<'a>(
        self: Box<Self>,
        dispatch: &'a mut UdpDispatch,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            let services = self.needs_proxy.then(|| ctx.runtime_services());
            execute_managed_datagram_operation(dispatch, services, session, payload, self.plan)
                .await
        })
    }
}

async fn execute_managed_datagram_operation<T>(
    dispatch: &mut UdpDispatch,
    services: Option<UdpRuntimeServices>,
    session: &Session,
    payload: &[u8],
    operation: zero_transport::managed_udp::ManagedDatagramStartPlan<T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
{
    dispatch
        .start_transport_managed_datagram(services, session, payload, operation)
        .await
}
