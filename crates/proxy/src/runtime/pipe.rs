//! Kernel pipe abstraction.
//!
//! The proxy runtime is an orchestration engine. This trait is the top-level
//! runtime boundary: TCP and UDP are the two core pipe implementations, while
//! concrete protocols plug into those pipes through protocol traits and
//! dispatch categories.

use zero_core::Session;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_core::{Address, InboundUdpDispatch, ProtocolType, SessionAuth};
use zero_engine::EngineError;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::TcpRouteResult;

/// Common runtime pipe boundary for kernel orchestration.
pub(crate) trait KernelPipe {
    type Input<'a>;
    type Output;
    type Error;

    async fn dispatch(&mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error>;
}

/// TCP connection pipe.
pub(crate) struct TcpPipe<'a> {
    proxy: &'a Proxy,
}

impl<'a> TcpPipe<'a> {
    pub(crate) fn new(proxy: &'a Proxy) -> Self {
        Self { proxy }
    }
}

/// Input for one TCP connection dispatch.
pub(crate) struct TcpPipeInput<'a> {
    pub(crate) session: &'a mut Session,
}

impl KernelPipe for TcpPipe<'_> {
    type Input<'a> = TcpPipeInput<'a>;
    type Output = TcpRouteResult;
    type Error = EngineError;

    async fn dispatch(&mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error> {
        self.proxy.dispatch_tcp(input.session).await
    }
}

/// Input for one UDP packet dispatch within an inbound UDP association.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpPipeInput<'a> {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
    pub(crate) protocol: ProtocolType,
    pub(crate) auth: Option<&'a SessionAuth>,
    /// Per-client-session isolation key (SIP022 3.2.4).
    ///
    /// When `Some`, flows that would collide on `(target, port)` alone are
    /// treated as independent relay sessions.  The Shadowsocks 2022 inbound
    /// passes the client's SIP022 session id here; all other protocols pass
    /// `None`.
    pub(crate) client_session_id: Option<u64>,
}

/// UDP datagram pipe.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpPipe<'a> {
    proxy: &'a Proxy,
    dispatch: &'a mut UdpDispatch,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl<'a> UdpPipe<'a> {
    pub(crate) fn new(proxy: &'a Proxy, dispatch: &'a mut UdpDispatch) -> Self {
        Self { proxy, dispatch }
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl KernelPipe for UdpPipe<'_> {
    type Input<'a> = UdpPipeInput<'a>;
    type Output = u64;
    type Error = EngineError;

    async fn dispatch(&mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error> {
        UdpDispatch::dispatch(self.dispatch, self.proxy, input).await
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl<'a> UdpPipeInput<'a> {
    pub(crate) fn from_inbound_dispatch(
        dispatch: &'a InboundUdpDispatch,
        auth: Option<&'a SessionAuth>,
    ) -> Self {
        Self {
            target: dispatch.target().clone(),
            port: dispatch.port(),
            payload: dispatch.payload(),
            protocol: dispatch.protocol(),
            auth,
            client_session_id: dispatch.client_session_id(),
        }
    }
}
