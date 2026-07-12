use super::handler::UpstreamAssociationHandler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UpstreamAssociationStages {
    pub(crate) proxy_stage: &'static str,
    pub(crate) resume_stage: &'static str,
    pub(crate) resume_message: &'static str,
}

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
