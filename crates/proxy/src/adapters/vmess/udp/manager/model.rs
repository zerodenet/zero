use zero_core::{Address, Session};

use crate::adapters::vmess::mux_pool::VmessMuxConnectionPool;
use crate::runtime::Proxy;

#[derive(Clone)]
pub(super) struct VmessUdpUpstream {
    pub(super) session_id: u64,
    pub(super) sender: vmess::VmessUdpFlowSender,
}

pub(crate) struct VmessUdpStartFlow<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) mux_pool: &'a VmessMuxConnectionPool,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) identity: vmess::VmessUdpIdentity,
    pub(crate) cipher_name: &'a str,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) transport: crate::transport::VmessTransportOptions<'a>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessUdpRelayFlowStart<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) identity: vmess::VmessUdpIdentity,
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
    pub(super) identity: vmess::VmessUdpIdentity,
    pub(super) cipher_name: &'a str,
    pub(super) initial_payload: &'a [u8],
    pub(super) transport: Option<&'a crate::transport::VmessTransportOptions<'a>>,
    pub(super) mux_concurrency: Option<u32>,
}
