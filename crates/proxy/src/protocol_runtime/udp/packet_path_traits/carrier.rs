use async_trait::async_trait;
use std::sync::Arc;
use zero_core::Address;
use zero_engine::EngineError;

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(crate) use zero_traits::DatagramCodec;

/// Object-safe packet-path carrier.
///
/// Each concrete carrier implements this so the packet-path manager can hold a
/// `Arc<dyn PacketPathCarrier>` without a per-protocol enum. Adapters build the
/// concrete carrier and box it; adding a carrier = implement this trait + the
/// adapter's `build_udp_packet_path`, zero manager changes.
#[async_trait]
pub(crate) trait PacketPathCarrier: Send + Sync {
    /// Send `payload` to `target:port` through this carrier.
    async fn send_to(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<(), EngineError>;

    /// Receive the next datagram, stripping transport framing.
    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

/// Carrier identity for cache lookup (cheap, computed before dialing).
///
/// Produced by `UdpPacketPathCapability::udp_packet_path_carrier_descriptor`. The
/// `cache_key` uniquely identifies one carrier connection so the manager can
/// reuse it across packets; `server`/`port` are the endpoint for diagnostics.
pub(crate) struct PacketPathCarrierDescriptor {
    pub(crate) cache_key: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

/// Datagram source params for a relay-chain final hop over a packet path.
///
/// Produced by `UdpPacketPathCapability::udp_datagram_source`. The `cache_key`
/// feeds packet-path cache identity without exposing raw config parsing to the
/// manager.
pub(crate) struct UdpDatagramDescriptor<'a> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
}

impl UdpDatagramDescriptor<'_> {
    pub(crate) fn key_part(&self) -> UdpDatagramKey {
        UdpDatagramKey {
            tag: self.tag.to_owned(),
            server: self.server.to_owned(),
            port: self.port,
            cache_key: self.cache_key.clone(),
        }
    }
}

/// Adapter-provided datagram role output for packet-path relay chains.
///
/// The descriptor is the generic chain-management surface. The codec is the
/// protocol-provided packet framing object for the selected datagram hop.
pub(crate) struct UdpDatagramSource<'a> {
    pub(crate) descriptor: UdpDatagramDescriptor<'a>,
    pub(crate) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
}

impl UdpDatagramSource<'_> {
    pub(crate) fn descriptor(&self) -> &UdpDatagramDescriptor<'_> {
        &self.descriptor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UdpDatagramKey {
    pub(crate) tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathLookupKey {
    pub(crate) carrier_cache_key: String,
    pub(crate) datagram: UdpDatagramKey,
}

impl PacketPathLookupKey {
    pub(crate) fn from_parts(
        carrier: &PacketPathCarrierDescriptor,
        datagram: UdpDatagramKey,
    ) -> Self {
        Self {
            carrier_cache_key: carrier.cache_key.clone(),
            datagram,
        }
    }

    pub(crate) fn datagram_endpoint(&self) -> (String, u16) {
        (self.datagram.server.clone(), self.datagram.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathFlowSnapshot {
    pub(crate) carrier_cache_key: String,
    pub(crate) datagram_tag: String,
    pub(crate) datagram_server: String,
    pub(crate) datagram_port: u16,
    pub(crate) datagram_cache_key: String,
}

impl PacketPathFlowSnapshot {
    fn from_parts(
        datagram: &UdpDatagramDescriptor<'_>,
        carrier: &PacketPathCarrierDescriptor,
    ) -> Self {
        Self {
            carrier_cache_key: carrier.cache_key.clone(),
            datagram_tag: datagram.tag.to_owned(),
            datagram_server: datagram.server.to_owned(),
            datagram_port: datagram.port,
            datagram_cache_key: datagram.cache_key.clone(),
        }
    }

    pub(crate) fn lookup_key(&self) -> PacketPathLookupKey {
        PacketPathLookupKey {
            carrier_cache_key: self.carrier_cache_key.clone(),
            datagram: UdpDatagramKey {
                tag: self.datagram_tag.clone(),
                server: self.datagram_server.clone(),
                port: self.datagram_port,
                cache_key: self.datagram_cache_key.clone(),
            },
        }
    }
}

pub(crate) struct PacketPathFlowBinding<'a> {
    datagram: UdpDatagramSource<'a>,
    flow_snapshot: PacketPathFlowSnapshot,
}

impl<'a> PacketPathFlowBinding<'a> {
    pub(crate) fn new(
        datagram: UdpDatagramSource<'a>,
        carrier_desc: &PacketPathCarrierDescriptor,
    ) -> Self {
        let flow_snapshot = PacketPathFlowSnapshot::from_parts(datagram.descriptor(), carrier_desc);
        Self {
            datagram,
            flow_snapshot,
        }
    }

    pub(crate) fn into_parts(self) -> (UdpDatagramSource<'a>, PacketPathFlowSnapshot) {
        (self.datagram, self.flow_snapshot)
    }
}
