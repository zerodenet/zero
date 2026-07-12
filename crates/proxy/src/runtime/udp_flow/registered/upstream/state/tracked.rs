pub(in crate::runtime::udp_flow::registered::upstream) struct TrackedUpstreamAssociation<T, A> {
    target: T,
    association: A,
}

pub(crate) struct TrackedUpstreamAssociationState<T, A> {
    upstream: Option<TrackedUpstreamAssociation<T, A>>,
}

impl<T, A> TrackedUpstreamAssociation<T, A> {
    pub(in crate::runtime::udp_flow::registered::upstream) fn new(
        target: T,
        association: A,
    ) -> Self {
        Self {
            target,
            association,
        }
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn target(&self) -> &T {
        &self.target
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn association(&self) -> &A {
        &self.association
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn into_parts(self) -> (T, A) {
        (self.target, self.association)
    }
}

impl<T, A> TrackedUpstreamAssociationState<T, A> {
    pub(crate) fn new() -> Self {
        Self { upstream: None }
    }

    pub(crate) fn target(&self) -> Option<&T> {
        self.upstream
            .as_ref()
            .map(TrackedUpstreamAssociation::target)
    }

    pub(crate) fn association(&self) -> Option<&A> {
        self.upstream
            .as_ref()
            .map(TrackedUpstreamAssociation::association)
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn insert(
        &mut self,
        target: T,
        association: A,
    ) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.track(TrackedUpstreamAssociation::new(target, association))
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn track(
        &mut self,
        upstream: TrackedUpstreamAssociation<T, A>,
    ) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.upstream.replace(upstream)
    }

    pub(in crate::runtime::udp_flow::registered::upstream) fn take(
        &mut self,
    ) -> Option<TrackedUpstreamAssociation<T, A>> {
        self.upstream.take()
    }
}

impl<T, A> Default for TrackedUpstreamAssociationState<T, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, A> TrackedUpstreamAssociationState<T, A>
where
    T: PartialEq,
{
    pub(crate) fn matches_target(&self, target: &T) -> bool {
        self.upstream
            .as_ref()
            .map(|upstream| upstream.target() == target)
            .unwrap_or(false)
    }
}
