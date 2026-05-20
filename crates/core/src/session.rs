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
    Vless,
    Hysteria2,
    Shadowsocks,
    Trojan,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAuth {
    pub scheme: String,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
}

impl SessionAuth {
    pub fn new(scheme: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            credential_id: None,
            principal_key: None,
        }
    }
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
    pub auth: Option<SessionAuth>,
    /// TLS Server Name Indication from ClientHello, if peeked.
    pub sni: Option<String>,
    /// Client's source IP, if available from the inbound listener.
    pub source_ip: Option<Address>,
    /// Client's source port, if available.
    pub source_port: Option<u16>,
    /// Local process ID that initiated this connection (Linux only).
    pub process_id: Option<u32>,
    /// Local process name (Linux only).
    pub process_name: Option<String>,
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
            auth: None,
            sni: None,
            source_ip: None,
            source_port: None,
            process_id: None,
            process_name: None,
        }
    }
}
