use std::time::Duration;

use tokio::time::Instant as TokioInstant;

use super::super::super::contract::UpstreamAssociationTarget;
use super::super::super::state::{TrackedUpstreamAssociation, TrackedUpstreamAssociationState};

pub(crate) struct UpstreamAssociationRuntime<T, A> {
    pub(crate) upstream: TrackedUpstreamAssociationState<T, A>,
    pub(super) idle_deadline: Option<TokioInstant>,
}

impl<T, A> UpstreamAssociationRuntime<T, A> {
    pub(crate) fn idle_deadline(&self) -> Option<TokioInstant> {
        self.idle_deadline
    }

    pub(crate) fn touch_idle(&mut self, timeout: Duration) {
        self.idle_deadline = Some(TokioInstant::now() + timeout);
    }

    pub(super) fn take_upstream(&mut self) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.idle_deadline = None;
        self.upstream.take()
    }
}

impl<T, A> Default for UpstreamAssociationRuntime<T, A> {
    fn default() -> Self {
        Self {
            upstream: TrackedUpstreamAssociationState::new(),
            idle_deadline: None,
        }
    }
}

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
{
    pub(crate) fn upstream_outbound_tag(&self) -> Option<&str> {
        self.upstream
            .target()
            .map(UpstreamAssociationTarget::outbound_tag)
    }
}
