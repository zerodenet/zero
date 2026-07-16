use super::super::super::contract::{
    UpstreamAssociationHandler, UpstreamAssociationStages, UpstreamAssociationTarget,
    UpstreamAssociationTransport,
};
use super::model::RegisteredUpstreamAssociationHandler;

pub(crate) fn boxed_registered_upstream_handler<T, A>(
    stages: UpstreamAssociationStages,
) -> Box<dyn UpstreamAssociationHandler>
where
    T: UpstreamAssociationTarget + Send + Sync + 'static,
    A: UpstreamAssociationTransport<T> + 'static,
{
    Box::new(RegisteredUpstreamAssociationHandler::<T, A>::new(stages))
}
