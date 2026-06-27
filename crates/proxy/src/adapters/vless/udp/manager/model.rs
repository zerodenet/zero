use zero_core::{Address, Session};

use crate::adapters::vless::mux_pool::MuxConnectionPool;
use crate::runtime::Proxy;

/// Handle to an established VLESS UDP upstream connection.
#[derive(Clone)]
pub(super) struct VlessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) session: vless::VlessUdpFlowSession,
}

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a MuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
    pub(crate) split_http: &'a zero_config::SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHopStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::VlessUdpFlowConfig<'a>,
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
    pub(super) config: vless::VlessUdpFlowConfig<'a>,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a crate::transport::VlessUdpTransportOptions<'a>>,
}
