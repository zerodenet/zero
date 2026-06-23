use tokio::sync::mpsc;
use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_core::{Address, Session};

use crate::runtime::Proxy;

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub(super) struct VlessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
}

/// Transport options for VLESS UDP upstream connections.
#[derive(Clone, Copy)]
pub(crate) struct VlessUdpTransport<'a> {
    pub(crate) tls: Option<&'a ClientTlsConfig>,
    pub(crate) reality: Option<&'a RealityConfig>,
    pub(crate) ws: Option<&'a WebSocketConfig>,
    pub(crate) grpc: Option<&'a GrpcConfig>,
    pub(crate) h2: Option<&'a H2Config>,
    pub(crate) http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a SplitHttpConfig>,
    pub(crate) quic: Option<&'a QuicConfig>,
}

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) id: &'a str,
    pub(crate) flow: Option<&'a str>,
    pub(crate) transport: VlessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) split_http: &'a SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHop<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) id: &'a str,
    pub(crate) transport: VlessUdpTransport<'a>,
    pub(crate) payload: &'a [u8],
}

pub(super) struct VlessUdpUpstreamRequest<'a> {
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) server: &'a str,
    pub(super) server_port: u16,
    pub(super) id: &'a str,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a VlessUdpTransport<'a>>,
}
