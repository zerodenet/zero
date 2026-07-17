use super::handler::UpstreamAssociationHandler;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use zero_core::Session;

pub(crate) struct UpstreamAssociationSend<'a> {
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "upstream-association-runtime")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

#[cfg(feature = "upstream-association-runtime")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UpstreamAssociationStages {
    pub(crate) proxy_stage: &'static str,
    pub(crate) resume_stage: &'static str,
    pub(crate) resume_message: &'static str,
}

#[cfg(feature = "upstream-association-runtime")]
impl UpstreamAssociationStages {
    pub(crate) const fn new(
        proxy_stage: &'static str,
        resume_stage: &'static str,
        resume_message: &'static str,
    ) -> Self {
        Self {
            proxy_stage,
            resume_stage,
            resume_message,
        }
    }
}

pub(crate) struct UpstreamUdpHandlers {
    pub(crate) upstream: Vec<Box<dyn UpstreamAssociationHandler>>,
}
