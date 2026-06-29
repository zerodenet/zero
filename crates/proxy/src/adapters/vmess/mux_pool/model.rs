use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::{Error, Session};

use crate::runtime::Proxy;

pub(crate) struct VmessMuxOpenRequest<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) identity: vmess::mux::VmessMuxIdentity,
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
    pub(crate) max_concurrency: u32,
}

impl VmessMuxOpenRequest<'_> {
    pub(crate) fn pool_key(&self) -> Result<vmess::mux::VmessMuxPoolKey, Error> {
        vmess::mux::VmessMuxPoolKey::from_config_parts(
            self.server.clone(),
            self.port,
            self.identity.clone(),
            self.tls.and_then(|tls| tls.server_name.as_deref()),
            self.ws.map(|ws| ws.path.as_str()),
            self.grpc.map(|grpc| grpc.service_names.clone()),
        )
    }
}

#[derive(Clone)]
pub(crate) struct VmessMuxConnectionPool {
    pub(super) pool:
        Arc<Mutex<HashMap<vmess::mux::VmessMuxPoolKey, Arc<vmess::mux::VmessMuxConn>>>>,
}
