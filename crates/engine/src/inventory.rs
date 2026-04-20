use zero_protocol_http_connect::HttpConnectInbound;
use zero_protocol_socks5::{Socks5Inbound, Socks5Outbound};

use crate::outbound::{BlockOutbound, DirectOutbound};

#[derive(Debug, Default, Clone, Copy)]
pub struct ProtocolInventory {
    pub socks5_inbound: Socks5Inbound,
    pub socks5_outbound: Socks5Outbound,
    pub http_connect_inbound: HttpConnectInbound,
    pub direct_outbound: DirectOutbound,
    pub block_outbound: BlockOutbound,
}

impl ProtocolInventory {
    pub fn supported_inbounds(&self) -> [&'static str; 3] {
        ["socks5", "http-connect", "mixed"]
    }

    pub fn supported_outbounds(&self) -> [&'static str; 3] {
        ["direct", "block", "socks5"]
    }
}
