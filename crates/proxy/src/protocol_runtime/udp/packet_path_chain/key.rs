use crate::protocol_runtime::udp::packet_path_traits::{
    PacketPathCarrierDescriptor, UdpDatagramKey,
};

/// Owned, hashable identity of one carrier+datagram packet-path connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PathKey {
    /// Adapter-provided carrier identity (e.g. `"socks5|host:port|user"`).
    pub(super) carrier_key: String,
    pub(super) datagram_tag: String,
    pub(super) datagram_server: String,
    pub(super) datagram_port: u16,
    pub(super) datagram_cache_key: String,
}

impl PathKey {
    pub(super) fn from_snapshot(
        carrier_cache_key: &str,
        datagram_tag: &str,
        datagram_server: &str,
        datagram_port: u16,
        datagram_cache_key: &str,
    ) -> Self {
        Self {
            carrier_key: carrier_cache_key.to_owned(),
            datagram_tag: datagram_tag.to_owned(),
            datagram_server: datagram_server.to_owned(),
            datagram_port,
            datagram_cache_key: datagram_cache_key.to_owned(),
        }
    }

    pub(super) fn from_sources(
        carrier: &PacketPathCarrierDescriptor,
        datagram: UdpDatagramKey,
    ) -> Self {
        Self {
            carrier_key: carrier.cache_key.clone(),
            datagram_tag: datagram.tag,
            datagram_server: datagram.server,
            datagram_port: datagram.port,
            datagram_cache_key: datagram.cache_key,
        }
    }
}
