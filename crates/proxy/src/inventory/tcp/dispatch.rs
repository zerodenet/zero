use zero_core::Session;
use zero_engine::ResolvedOutbound;

use super::{dispatch_prepared_tcp_candidate, dispatch_prepared_tcp_relay_chain};
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

pub(crate) async fn dispatch_tcp_outbound(
    services: TcpRuntimeServices,
    session: &Session,
    resolved: ResolvedOutbound<'static>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    match resolved {
        ResolvedOutbound::Relay { chain } => {
            let prepared = services.prepare_tcp_relay_chain(&chain)?;
            dispatch_prepared_tcp_relay_chain(services, session, prepared).await
        }
        ResolvedOutbound::Single(candidate) => {
            let prepared = services.prepare_tcp_candidate(&candidate)?;
            dispatch_prepared_tcp_candidate(services, session, prepared).await
        }
        ResolvedOutbound::Fallback { candidates } => {
            let mut last_failure = None;

            for candidate in candidates {
                let prepared = match services.prepare_tcp_candidate(&candidate) {
                    Ok(prepared) => prepared,
                    Err(failure) => {
                        last_failure = Some(failure);
                        continue;
                    }
                };
                match dispatch_prepared_tcp_candidate(services.clone(), session, prepared).await {
                    Ok(outbound) => return Ok(outbound),
                    Err(failure) => last_failure = Some(failure),
                }
            }

            Err(last_failure.expect("validated fallback groups always have at least one candidate"))
        }
    }
}
