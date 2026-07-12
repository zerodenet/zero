use crate::runtime::Proxy;

/// Wraps [`EngineHandle`] with TUN command interception.
///
/// TUN start/stop commands are handled by the proxy runtime,
/// not the engine. This wrapper intercepts those commands
/// before they reach `EngineHandle`.
#[derive(Clone)]
pub struct ProxyHandle {
    pub(super) inner: zero_engine::EngineHandle,
    pub(super) proxy: Proxy,
}

impl ProxyHandle {
    pub fn new(inner: zero_engine::EngineHandle, proxy: Proxy) -> Self {
        Self { inner, proxy }
    }

    /// Access the underlying EngineHandle.
    pub fn engine_handle(&self) -> &zero_engine::EngineHandle {
        &self.inner
    }
}
