use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::Session;

use crate::runtime::Proxy;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct VmessMuxPoolKey {
    pub(super) server: String,
    pub(super) port: u16,
    pub(super) id: [u8; 16],
    pub(super) cipher: String,
    pub(super) transport: VmessMuxTransportKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum VmessMuxTransportKey {
    RawTls {
        server_name: Option<String>,
    },
    Ws {
        server_name: Option<String>,
        path: String,
    },
    Grpc {
        server_name: Option<String>,
        service_names: Vec<String>,
    },
}

pub(super) struct VmessMuxConn {
    pub(super) write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pub(super) streams: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    pub(super) next_id: Mutex<u16>,
    pub(super) active: Arc<Mutex<usize>>,
    pub(super) max_concurrency: u32,
}

pub(crate) struct VmessMuxOpenRequest<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) id: [u8; 16],
    pub(crate) cipher: String,
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
    pub(crate) max_concurrency: u32,
}

#[derive(Clone)]
pub(crate) struct VmessMuxConnectionPool {
    pub(super) pool: Arc<Mutex<HashMap<VmessMuxPoolKey, Arc<VmessMuxConn>>>>,
}
