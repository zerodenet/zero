use zero_core::Address;

use super::accounting::UdpInboundResponseAccounting;

#[cfg(feature = "socks5")]
pub(crate) struct UdpUpstreamResponseParts {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
    pub(crate) accounting: UdpInboundResponseAccounting,
}

pub(crate) struct UdpDirectResponseParts<'payload> {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: &'payload [u8],
    pub(crate) accounting: UdpInboundResponseAccounting,
}

pub(crate) struct UdpChainResponseParts {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
    pub(crate) accounting: UdpInboundResponseAccounting,
}
