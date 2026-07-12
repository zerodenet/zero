use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
use crate::runtime::Proxy;
use zero_core::Session;

#[derive(Clone, Copy)]
pub(super) enum ManagedUdpOutboundKind {
    Relay,
    Datagram,
    StreamPacket,
}

pub(super) struct ManagedUdpSend<'a> {
    pub(super) proxy: Option<&'a Proxy>,
    pub(super) tag: &'a str,
    pub(super) session: &'a Session,
    pub(super) carrier: Option<crate::transport::RelayCarrier>,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: ManagedUdpFlowResume,
    pub(super) payload: &'a [u8],
    pub(super) kind: ManagedUdpFlowKind,
    pub(super) outbound: ManagedUdpOutboundKind,
}

pub(crate) struct ManagedDatagramStart<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedRelayStart<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedStreamPacketStart<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
    pub(crate) payload: &'a [u8],
    pub(crate) relay_chain: bool,
}
