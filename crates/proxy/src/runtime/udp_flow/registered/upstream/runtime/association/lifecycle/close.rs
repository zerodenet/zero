use zero_engine::EngineError;

use super::super::super::super::contract::{
    UpstreamAssociationCloseReason, UpstreamAssociationTarget, UpstreamAssociationTransport,
};
use super::super::model::UpstreamAssociationRuntime;

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    pub(crate) fn drop_after_send_error(&mut self, inbound_tag: &str, error: &EngineError) {
        if let Some(assoc) = self.upstream.take() {
            let (record, association) = assoc.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            let (outbound_tag, server, port) = record.log_parts();
            crate::logging::log_udp_upstream_association_dropped(
                inbound_tag,
                outbound_tag,
                server,
                port,
                error,
            );
        }
        self.idle_deadline = None;
    }

    pub(crate) fn close_idle(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::IdleTimeout);
            target
        })
    }

    pub(crate) fn close_dropped(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            target
        })
    }

    pub(crate) fn close_all_upstreams(&mut self) {
        if let Some(association) = self.upstream.take() {
            let (_, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Closed);
        }
        self.idle_deadline = None;
    }
}
