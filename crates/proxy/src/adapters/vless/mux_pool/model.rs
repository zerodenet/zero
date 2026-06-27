use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vless::mux_pool::{MuxPoolConn, PoolKey};
use zero_config::{ClientTlsConfig, RealityConfig};
use zero_core::Session;

use crate::runtime::Proxy;

#[derive(Clone)]
pub(crate) struct MuxConnectionPool {
    pub(super) pool: Arc<Mutex<HashMap<PoolKey, Arc<MuxPoolConn>>>>,
}

pub(crate) struct VlessMuxOpenRequest<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: Option<&'a Session>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a [u8; 16],
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) reality: Option<&'a RealityConfig>,
    pub(crate) max_concurrency: u32,
}
