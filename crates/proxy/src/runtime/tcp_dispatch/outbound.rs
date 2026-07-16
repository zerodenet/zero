use zero_core::Session;
use zero_engine::ResolvedOutbound;

use crate::inventory::PreparedTcpOutbound;
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

use super::candidate::dispatch_prepared_tcp_candidate;

pub(crate) async fn dispatch_tcp_outbound(
    services: TcpRuntimeServices,
    session: &Session,
    resolved: ResolvedOutbound<'static>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    let prepared = services.prepare_tcp_outbound(&resolved)?;
    execute_prepared_tcp_outbound(services, session, prepared).await
}

async fn execute_prepared_tcp_outbound(
    services: TcpRuntimeServices,
    session: &Session,
    prepared: PreparedTcpOutbound<'_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    match prepared {
        PreparedTcpOutbound::Relay(prepared) => {
            super::relay::dispatch_prepared_tcp_relay_chain(services, session, prepared).await
        }
        PreparedTcpOutbound::Single(prepared) => {
            dispatch_prepared_tcp_candidate(services, session, prepared).await
        }
        PreparedTcpOutbound::Fallback(candidates) => {
            let mut last_failure = None;

            for prepared in candidates {
                match dispatch_prepared_tcp_candidate(services.clone(), session, prepared).await {
                    Ok(outbound) => return Ok(outbound),
                    Err(failure) => last_failure = Some(failure),
                }
            }

            Err(last_failure
                .expect("validated fallback groups always have at least one prepared candidate"))
        }
    }
}
