#[derive(Debug, Clone, Default)]
pub struct VmessTransportRuntime {
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub(super) fn mux_pool(&self) -> crate::mux::VmessMuxConnectionPool {
        self.mux_pool.clone()
    }
}
