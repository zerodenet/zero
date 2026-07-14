use zero_core::Session;
use zero_engine::ResolvedOutbound;

use super::super::ProtocolInventory;
use super::{dispatch_prepared_tcp_candidate, dispatch_prepared_tcp_relay_chain};
use crate::protocol_registry::OutboundAdapterContext;
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

pub(crate) enum PreparedTcpOutbound<'a> {
    Relay(super::PreparedTcpRelayChain<'a>),
    Single(super::PreparedTcpCandidate<'a>),
    Fallback(Vec<super::PreparedTcpCandidate<'a>>),
}

impl PreparedTcpOutbound<'_> {
    pub(crate) async fn execute(
        self,
        services: TcpRuntimeServices,
        session: &Session,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self {
            PreparedTcpOutbound::Relay(prepared) => {
                dispatch_prepared_tcp_relay_chain(services, session, prepared).await
            }
            PreparedTcpOutbound::Single(prepared) => {
                dispatch_prepared_tcp_candidate(services, session, prepared).await
            }
            PreparedTcpOutbound::Fallback(candidates) => {
                let mut last_failure = None;

                for prepared in candidates {
                    match dispatch_prepared_tcp_candidate(services.clone(), session, prepared).await
                    {
                        Ok(outbound) => return Ok(outbound),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure.expect(
                    "validated fallback groups always have at least one prepared candidate",
                ))
            }
        }
    }
}

impl ProtocolInventory {
    pub(crate) fn prepare_tcp_outbound<'a>(
        &self,
        ctx: OutboundAdapterContext,
        resolved: &'a ResolvedOutbound<'a>,
    ) -> Result<PreparedTcpOutbound<'a>, TcpOutboundFailure> {
        match resolved {
            ResolvedOutbound::Relay { chain } => Ok(PreparedTcpOutbound::Relay(
                self.prepare_tcp_relay_chain(ctx, chain)?,
            )),
            ResolvedOutbound::Single(candidate) => Ok(PreparedTcpOutbound::Single(
                self.prepare_tcp_candidate(ctx, candidate)?,
            )),
            ResolvedOutbound::Fallback { candidates } => {
                let mut prepared = Vec::with_capacity(candidates.len());
                let mut last_failure = None;

                for candidate in candidates {
                    match self.prepare_tcp_candidate(ctx.clone(), candidate) {
                        Ok(candidate) => prepared.push(candidate),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                if prepared.is_empty() {
                    Err(last_failure
                        .expect("validated fallback groups always have at least one candidate"))
                } else {
                    Ok(PreparedTcpOutbound::Fallback(prepared))
                }
            }
        }
    }
}

pub(crate) async fn dispatch_tcp_outbound(
    services: TcpRuntimeServices,
    session: &Session,
    resolved: ResolvedOutbound<'static>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    let prepared = services.prepare_tcp_outbound(&resolved)?;
    prepared.execute(services, session).await
}
