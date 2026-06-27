use zero_core::{Address, Session};

use crate::adapters::vmess::mux_pool::VmessMuxConnectionPool;
use crate::runtime::udp_flow::managed::BoxedManagedStreamUdpConnection;
use crate::runtime::Proxy;

pub(super) struct VmessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) connection: BoxedManagedStreamUdpConnection,
}

pub(crate) struct VmessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a VmessMuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) config: vmess::VmessUdpFlowConfig<'a>,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessUdpRelayFlowStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) config: vmess::VmessUdpFlowConfig<'a>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(super) struct VmessUdpUpstreamRequest<'a> {
    pub(super) proxy: &'a Proxy,
    pub(super) mux_pool: &'a VmessMuxConnectionPool,
    pub(super) session: &'a Session,
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) server: &'a str,
    pub(super) server_port: u16,
    pub(super) config: vmess::VmessUdpFlowConfig<'a>,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a crate::transport::VmessTransportOptions<'a>>,
    pub(super) mux_concurrency: Option<u32>,
}
