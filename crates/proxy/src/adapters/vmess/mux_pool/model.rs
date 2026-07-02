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
        vmess::mux::VmessMuxPoolKeyConfig::new(
            self.server.clone(),
            self.port,
            self.identity.clone(),
        )
        .with_tls_server_name(self.tls.and_then(|tls| tls.server_name.as_deref()))
        .with_ws_path(self.ws.map(|ws| ws.path.as_str()))
        .with_grpc_service_names(self.grpc.map(|grpc| grpc.service_names.clone()))
        .into_pool_key()
    }
}
