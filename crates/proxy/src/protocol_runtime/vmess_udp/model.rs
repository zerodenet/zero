use tokio::sync::mpsc;
use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::{Address, Session};

use crate::runtime::Proxy;

#[derive(Clone)]
pub(super) struct VmessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
}

#[derive(Clone, Copy)]
pub(crate) struct VmessUdpTransport<'a> {
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
}

pub(crate) struct VmessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) uuid: [u8; 16],
    pub(crate) cipher_name: &'a str,
    pub(crate) cipher: vmess::VmessCipher,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) transport: VmessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessUdpRelayFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) uuid: [u8; 16],
    pub(crate) cipher: vmess::VmessCipher,
    pub(crate) transport: VmessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(super) struct VmessUdpUpstreamRequest<'a> {
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) server: &'a str,
    pub(super) server_port: u16,
    pub(super) uuid: [u8; 16],
    pub(super) cipher_name: &'a str,
    pub(super) cipher: vmess::VmessCipher,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a VmessUdpTransport<'a>>,
    pub(super) mux_concurrency: Option<u32>,
}
