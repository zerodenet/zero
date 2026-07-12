use std::any::Any;
use std::sync::Arc;

trait ManagedUdpFlowResumeObject: Any + Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}

impl<T> ManagedUdpFlowResumeObject for T
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedUdpFlowResume {
    inner: Arc<dyn ManagedUdpFlowResumeObject>,
}

impl ManagedUdpFlowResume {
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
        self.inner.as_ref().as_any().downcast_ref::<T>()
    }

    pub(crate) fn cloned<T>(self) -> Option<T>
    where
        T: Any + Clone,
    {
        self.as_ref::<T>().cloned()
    }
}
