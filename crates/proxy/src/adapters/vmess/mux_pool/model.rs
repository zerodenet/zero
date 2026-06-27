use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::Session;

use crate::runtime::Proxy;

pub(crate) struct VmessMuxOpenRequest<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) id: [u8; 16],
    pub(crate) cipher_name: String,
    pub(crate) cipher: vmess::VmessCipher,
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
    pub(crate) max_concurrency: u32,
}

#[derive(Clone)]
pub(crate) struct VmessMuxConnectionPool {
    pub(super) pool: Arc<Mutex<HashMap<vmess::VmessMuxPoolKey, Arc<vmess::VmessMuxConn>>>>,
}
