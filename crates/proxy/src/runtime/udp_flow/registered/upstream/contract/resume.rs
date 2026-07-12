use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;

pub(crate) fn handles_registered_resume<T>(resume: &ManagedUdpFlowResume) -> bool
where
    T: 'static,
{
    resume.as_ref::<T>().is_some()
}
