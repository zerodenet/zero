use zero_core::Session;
use zero_engine::{EngineError, ResolvedOutbound};

use super::super::{ClaimedResolvedOutbound, ProtocolInventory};
use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

enum PreparedUdpOutbound<'a> {
    Relay(Box<super::relay::PreparedUdpRelayChain<'a>>),
    Single(super::leaf::PreparedUdpLeafCandidate<'a>),
    Fallback(Vec<super::leaf::PreparedUdpLeafCandidate<'a>>),
}

impl PreparedUdpOutbound<'_> {
    pub(crate) async fn execute(
        self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        match self {
            PreparedUdpOutbound::Relay(prepared) => {
                prepared.execute(dispatch, ctx, session, payload).await
            }
            PreparedUdpOutbound::Single(prepared) => {
                prepared.execute(dispatch, ctx, session, payload).await
            }
            PreparedUdpOutbound::Fallback(candidates) => {
                let mut last_failure = None;

                for prepared in candidates {
                    match prepared
                        .execute(dispatch, ctx.clone(), session, payload)
                        .await
                    {
                        Ok(result) => return Ok(result),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                Err(last_failure.unwrap_or_else(|| FlowFailure {
                    stage: "fallback_exhausted",
                    error: EngineError::Io(std::io::Error::other("all fallback outbounds failed")),
                    upstream: None,
                }))
            }
        }
    }
}

impl ProtocolInventory {
    async fn prepare_claimed_udp_outbound<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        claimed: ClaimedResolvedOutbound<'a>,
        payload: &'a [u8],
    ) -> Result<PreparedUdpOutbound<'a>, FlowFailure> {
        match claimed {
            ClaimedResolvedOutbound::Single(candidate) => Ok(PreparedUdpOutbound::Single(
                self.prepare_claimed_udp_leaf_candidate(ctx, &candidate)?,
            )),
            ClaimedResolvedOutbound::Fallback(candidates) => {
                let mut prepared = Vec::with_capacity(candidates.len());
                let mut last_failure = None;

                for candidate in candidates {
                    match self.prepare_claimed_udp_leaf_candidate(ctx.clone(), &candidate) {
                        Ok(candidate) => prepared.push(candidate),
                        Err(failure) => last_failure = Some(failure),
                    }
                }

                if prepared.is_empty() {
                    Err(last_failure.unwrap_or_else(|| FlowFailure {
                        stage: "fallback_exhausted",
                        error: EngineError::Io(std::io::Error::other(
                            "all fallback outbounds failed",
                        )),
                        upstream: None,
                    }))
                } else {
                    Ok(PreparedUdpOutbound::Fallback(prepared))
                }
            }
            ClaimedResolvedOutbound::Relay(chain) => Ok(PreparedUdpOutbound::Relay(Box::new(
                self.prepare_claimed_udp_relay_chain(ctx, session, &chain, payload)
                    .await?,
            ))),
        }
    }

    async fn prepare_udp_outbound<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        resolved: &'a ResolvedOutbound<'a>,
        payload: &'a [u8],
    ) -> Result<PreparedUdpOutbound<'a>, FlowFailure> {
        let claimed = self.claim_udp_outbound(resolved)?;
        self.prepare_claimed_udp_outbound(ctx, session, claimed, payload)
            .await
    }
}

pub(crate) async fn start_udp_resolved_outbound(
    inventory: &ProtocolInventory,
    dispatch: &mut UdpDispatch,
    ctx: UdpAdapterContext<'_>,
    session: &Session,
    resolved: ResolvedOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let prepared = inventory
        .prepare_udp_outbound(ctx.clone(), session, &resolved, payload)
        .await?;
    prepared.execute(dispatch, ctx, session, payload).await
}
