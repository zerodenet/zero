use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum ProtocolUdpFlowSnapshot {
    Managed { resume: ProtocolUdpFlowResume },
}

trait ProtocolUdpFlowResumeObject: Any + Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}

impl<T> ProtocolUdpFlowResumeObject for T
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProtocolUdpFlowResume {
    inner: Arc<dyn ProtocolUdpFlowResumeObject>,
}

impl ProtocolUdpFlowResume {
    pub(crate) fn new<T>(resume: T) -> Self
    where
        T: Any + Send + Sync + std::fmt::Debug,
    {
        Self {
            inner: Arc::new(resume),
        }
    }

    pub(crate) fn as_ref<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.inner.as_any().downcast_ref::<T>()
    }

    pub(crate) fn cloned<T>(self) -> Option<T>
    where
        T: Any + Clone,
    {
        self.as_ref::<T>().cloned()
    }
}

impl ProtocolUdpFlowSnapshot {
    pub(crate) fn managed(resume: ProtocolUdpFlowResume) -> Self {
        Self::Managed { resume }
    }

    pub(crate) fn resume(&self) -> &ProtocolUdpFlowResume {
        match self {
            Self::Managed { resume } => resume,
        }
    }
}
