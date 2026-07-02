use vless::mux_pool::{PoolKey, PoolKeyConfig};
use zero_config::{ClientTlsConfig, RealityConfig};
use zero_core::Session;

use crate::runtime::Proxy;

pub(crate) struct VlessMuxOpenRequest<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: Option<&'a Session>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) identity: vless::mux_pool::MuxIdentity,
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) reality: Option<&'a RealityConfig>,
    pub(crate) max_concurrency: u32,
}

impl VlessMuxOpenRequest<'_> {
    pub(crate) fn pool_key(&self) -> PoolKey {
        PoolKeyConfig::new(self.server, self.port, self.identity.clone())
            .with_tls_server_name(self.tls.and_then(|tls| tls.server_name.as_deref()))
            .with_reality(
                self.reality.map(|reality| reality.public_key.as_str()),
                self.reality
                    .and_then(|reality| reality.server_name.as_deref()),
            )
            .into_pool_key()
    }
}
