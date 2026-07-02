use zero_core::Session;

use crate::runtime::Proxy;

pub(crate) struct VmessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a vmess::mux::VmessMuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vmess::udp::VmessUdpFlowConfig<'a>,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessUdpRelayFlowStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vmess::udp::VmessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}
