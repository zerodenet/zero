use zero_config::SplitHttpConfig;
use zero_core::{Address, Session};

use super::VlessFlowSender;
use crate::runtime::Proxy;

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub(super) struct VlessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) sender: VlessFlowSender,
}

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) identity: vless::VlessUdpIdentity,
    pub(crate) flow: Option<&'a str>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) identity: vless::VlessUdpIdentity,
    pub(crate) flow: Option<&'a str>,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) quic: Option<&'a zero_config::QuicConfig>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) identity: vless::VlessUdpIdentity,
    pub(crate) split_http: &'a SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHop<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) identity: vless::VlessUdpIdentity,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHopStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) identity: vless::VlessUdpIdentity,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(super) struct VlessUdpUpstreamRequest<'a> {
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) server: &'a str,
    pub(super) server_port: u16,
    pub(super) identity: vless::VlessUdpIdentity,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a crate::transport::VlessUdpTransportOptions<'a>>,
}
