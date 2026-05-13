//! VLESS UDP types.
//!
//! Shared types for VLESS UDP outbound management. Originally lived in
//! `crates/proxy/src/outbound/vless.rs`.

use tokio::sync::mpsc;
use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    WebSocketConfig,
};

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub struct VlessUdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

/// Transport options for VLESS UDP upstream connections.
#[derive(Clone, Copy)]
pub struct VlessUdpTransport<'a> {
    pub tls: Option<&'a ClientTlsConfig>,
    pub reality: Option<&'a RealityConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
    pub http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub quic: Option<&'a QuicConfig>,
}
