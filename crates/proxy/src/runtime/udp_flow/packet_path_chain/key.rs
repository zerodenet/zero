use crate::runtime::udp_flow::packet_path::{
    PacketPathCarrierDescriptor, PacketPathLookupKey, UdpDatagramKey,
};

/// Owned, hashable identity of one carrier+datagram packet-path connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PathKey {
    /// Adapter-provided carrier identity (e.g. `"carrier|host:port|identity"`).
    pub(super) carrier_key: String,
    pub(super) datagram: UdpDatagramKey,
}

impl PathKey {
    pub(super) fn from_lookup(lookup: PacketPathLookupKey) -> Self {
        let (carrier_key, datagram) = lookup.into_path_parts();
        Self {
            carrier_key,
            datagram,
        }
    }

    pub(super) fn from_sources(
        carrier: &PacketPathCarrierDescriptor,
        datagram: UdpDatagramKey,
    ) -> Self {
        Self::from_lookup(PacketPathLookupKey::from_parts(carrier, datagram))
    }
}
