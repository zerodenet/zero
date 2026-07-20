use alloc::string::String;

use crate::address::Address;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolType(&'static str);

impl ProtocolType {
    pub const UNKNOWN: Self = Self("unknown");

    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAuth {
    pub scheme: String,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub up_bps: Option<u64>,
    pub down_bps: Option<u64>,
}

impl SessionAuth {
    pub fn new(scheme: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            credential_id: None,
            principal_key: None,
            up_bps: None,
            down_bps: None,
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
    /// Per-connection upload rate limit in bytes/s. `None` = unlimited.
    pub up_bps: Option<u64>,
    /// Per-connection download rate limit in bytes/s. `None` = unlimited.
    pub down_bps: Option<u64>,
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
    /// Local process executable path (Linux only).
    pub process_path: Option<String>,
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
            up_bps: None,
            down_bps: None,
            sni: None,
            source_ip: None,
            source_port: None,
            process_id: None,
            process_name: None,
            process_path: None,
        }
    }

    /// Apply authenticated user identity and rate limits to this session.
    ///
    /// Every protocol handler should call this once after authentication,
    /// before `prepare_session`.  All the common wiring (principal_key,
    /// up_bps, down_bps) happens here in one place.
    pub fn apply_auth(&mut self, sa: SessionAuth) {
        self.up_bps = sa.up_bps;
        self.down_bps = sa.down_bps;
        self.auth = Some(sa);
    }
}
