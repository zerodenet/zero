use alloc::string::String;

use crate::address::Address;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolType {
    Socks5,
    HttpConnect,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: Address,
    pub port: u16,
    pub network: Network,
    pub protocol: ProtocolType,
}

impl Session {
    pub fn new(
        id: u64,
        target: Address,
        port: u16,
        network: Network,
        protocol: ProtocolType,
    ) -> Self {
        Self {
            id,
            inbound_tag: None,
            outbound_tag: None,
            target,
            port,
            network,
            protocol,
        }
    }
}
