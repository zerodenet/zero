#[derive(Debug, Clone, Default)]
pub struct VlessTransportRuntime {
    mux_pool: crate::mux_pool::MuxConnectionPool,
}

impl VlessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub(super) fn mux_pool(&self) -> crate::mux_pool::MuxConnectionPool {
        self.mux_pool.clone()
    }
}
