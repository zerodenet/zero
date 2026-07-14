use super::super::ProtocolInventory;
use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

pub(crate) enum PreparedUdpLeafCandidate<'a> {
    Block { tag: String },
    Flow(Box<dyn PreparedUdpFlowOperation + 'a>),
}

impl PreparedUdpLeafCandidate<'_> {
    pub(crate) async fn execute(
        self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        match self {
            PreparedUdpLeafCandidate::Block { tag } => Ok(FlowStartResult::Blocked { tag }),
            PreparedUdpLeafCandidate::Flow(operation) => {
                operation.execute(dispatch, ctx, session, payload).await
            }
        }
    }
}

impl ProtocolInventory {
    pub(crate) fn prepare_udp_leaf_candidate<'a>(
        &self,
        ctx: UdpAdapterContext<'a>,
        leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<PreparedUdpLeafCandidate<'a>, FlowFailure> {
        let claimed = self
            .claim_outbound_leaf(leaf)
            .map_err(|error| FlowFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream: None,
            })?;
        let runtime = claimed.runtime;
        if !ctx.udp_enabled_for_outbound(runtime.udp_policy_tag) {
            return Err(FlowFailure {
                stage: "udp_policy",
                error: zero_engine::EngineError::Io(std::io::Error::other(
                    "udp disabled for outbound",
                )),
                upstream: runtime
                    .endpoint
                    .map(|endpoint| (endpoint.server.to_owned(), endpoint.port)),
            });
        }
        if matches!(runtime.tcp_path, TcpPathCategory::Block) {
            return Ok(PreparedUdpLeafCandidate::Block {
                tag: runtime.kernel_tag.unwrap_or("block").to_string(),
            });
        }

        let operation = claimed.prepare_udp_flow(leaf, ctx.source_dir())?;
        Ok(PreparedUdpLeafCandidate::Flow(operation))
    }

    /// Start a single-hop UDP flow through the adapter that owns `leaf`.
    pub(crate) async fn start_udp_leaf_flow(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let prepared = self.prepare_udp_leaf_candidate(ctx.clone(), leaf)?;
        prepared.execute(dispatch, ctx, session, payload).await
    }
}
