use zero_core::Session;

use crate::runtime::Proxy;

pub(crate) struct VlessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a vless::mux_pool::MuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vless::udp::VlessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayTwoStream<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::udp::VlessUdpFlowConfig<'a>,
    pub(crate) split_http: &'a zero_config::SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessUdpRelayFinalHopStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vless::udp::VlessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VlessUdpTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}
