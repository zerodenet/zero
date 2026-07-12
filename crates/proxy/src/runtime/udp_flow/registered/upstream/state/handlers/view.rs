use tokio::time::Instant as TokioInstant;

use super::model::UpstreamAssociationState;

impl UpstreamAssociationState {
    pub(in crate::runtime::udp_flow::registered) fn upstream_outbound_tag(&self) -> Option<&str> {
        self.handlers
            .upstream
            .iter()
            .find_map(|handler| handler.upstream_outbound_tag())
    }

    pub(in crate::runtime::udp_flow::registered) fn upstream_idle_deadline(
        &self,
    ) -> Option<TokioInstant> {
        self.handlers
            .upstream
            .iter()
            .filter_map(|handler| handler.upstream_idle_deadline())
            .min()
    }
}
