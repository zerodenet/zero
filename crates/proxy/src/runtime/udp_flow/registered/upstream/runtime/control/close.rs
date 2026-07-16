use super::super::super::contract::{UpstreamAssociationTarget, UpstreamAssociationTransport};
use super::super::association::UpstreamAssociationRuntime;

pub(crate) fn close_registered_dropped_upstream<T, A>(
    runtime: &mut UpstreamAssociationRuntime<T, A>,
) -> Option<(String, String, u16)>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    runtime.close_dropped().map(registered_target_log_parts)
}

pub(crate) fn close_registered_idle_upstream<T, A>(
    runtime: &mut UpstreamAssociationRuntime<T, A>,
) -> Option<(String, String, u16)>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    runtime.close_idle().map(registered_target_log_parts)
}

fn registered_target_log_parts<T>(target: T) -> (String, String, u16)
where
    T: UpstreamAssociationTarget,
{
    let (outbound_tag, server, port) = target.log_parts();
    (outbound_tag.to_owned(), server.to_owned(), port)
}
