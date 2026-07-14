use zero_core::Session;
use zero_engine::{EngineError, ResolvedOutbound};

use super::super::ProtocolInventory;
use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

pub(crate) async fn start_udp_resolved_outbound(
    inventory: &ProtocolInventory,
    dispatch: &mut UdpDispatch,
    ctx: UdpAdapterContext<'_>,
    session: &Session,
    resolved: ResolvedOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    match resolved {
        ResolvedOutbound::Single(candidate) => {
            inventory
                .start_udp_leaf_flow(dispatch, ctx, session, &candidate, payload)
                .await
        }
        ResolvedOutbound::Fallback { candidates } => {
            let mut last_failure = None;

            for candidate in candidates {
                match inventory
                    .start_udp_leaf_flow(dispatch, ctx.clone(), session, &candidate, payload)
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
        ResolvedOutbound::Relay { chain } => {
            inventory
                .start_udp_relay_chain(dispatch, ctx, session, chain, payload)
                .await
        }
    }
}
