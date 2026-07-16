use super::super::super::contract::UpstreamAssociationStages;
use super::super::association::UpstreamAssociationRuntime;

pub(crate) struct RegisteredUpstreamAssociationHandler<T, A> {
    pub(super) runtime: UpstreamAssociationRuntime<T, A>,
    pub(super) stages: UpstreamAssociationStages,
}

impl<T, A> RegisteredUpstreamAssociationHandler<T, A> {
    pub(crate) fn new(stages: UpstreamAssociationStages) -> Self {
        Self {
            runtime: UpstreamAssociationRuntime::<T, A>::default(),
            stages,
        }
    }
}
