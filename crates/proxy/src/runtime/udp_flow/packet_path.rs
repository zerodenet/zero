//! Generic UDP packet-path flow abstractions.
//!
//! These types describe proxy-owned UDP relay-chain orchestration: response
//! tasks, flow context, packet references, packet-path carriers, datagram
//! sources, and neutral packet-path flow snapshots. Protocol crates and
//! adapters provide concrete codecs/carriers; the runtime schedules and tracks
//! the resulting flows.

use async_trait::async_trait;
use std::sync::Arc;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use std::vec::Vec;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via UDP dispatch chain polling.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

/// Runtime context shared by UDP outbound managers for one send operation.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct UdpFlowContext<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
}

/// Borrowed target payload for one UDP send operation.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Clone, Copy)]
pub(crate) struct UdpPacketRef<'a> {
    pub(crate) target: &'a Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
}

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(crate) use zero_traits::DatagramCodec;

/// Object-safe packet-path carrier.
///
/// Each concrete carrier implements this so the packet-path manager can hold a
/// `Arc<dyn PacketPathCarrier>` without a per-protocol enum. Adapters build the
/// concrete carrier and box it; adding a carrier = implement this trait + the
/// adapter's prepared packet-path operation, zero manager changes.
#[async_trait]
pub(crate) trait PacketPathCarrier: Send + Sync {
    /// Send `payload` to `target:port` through this carrier.
    async fn send_to(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<(), EngineError>;

    /// Receive the next datagram, stripping transport framing.
    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

/// Generic payload-based packet-path transport bridge.
///
/// Some protocol adapters expose an already-established upstream transport that
/// can send target-addressed payloads and return stripped payloads directly.
/// Runtime wraps those transports into a neutral `PacketPathCarrier` so
/// adapters do not need to define one-off carrier structs.
#[async_trait]
#[cfg(feature = "socks5")]
pub(crate) trait PacketPathPayloadTransport: Send + Sync {
    async fn send_to(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<(), EngineError>;

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

#[cfg(feature = "socks5")]
struct PacketPathPayloadCarrier(Arc<dyn PacketPathPayloadTransport>);

#[async_trait]
#[cfg(feature = "socks5")]
impl PacketPathCarrier for PacketPathPayloadCarrier {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.0.send_to(target, port, payload).await
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.0.recv_from(buf).await
    }
}

#[cfg(feature = "socks5")]
pub(crate) fn packet_path_payload_carrier(
    transport: Arc<dyn PacketPathPayloadTransport>,
) -> Arc<dyn PacketPathCarrier> {
    Arc::new(PacketPathPayloadCarrier(transport))
}

/// Carrier identity for cache lookup (cheap, computed before dialing).
///
/// Produced by `PreparedUdpPacketPathOperation::into_carrier_descriptor`. The
/// `cache_key` uniquely identifies one carrier connection so the manager can
/// reuse it across packets; `server`/`port` are the endpoint for diagnostics.
#[derive(Clone)]
pub(crate) struct PacketPathCarrierDescriptor {
    pub(crate) cache_key: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

#[cfg(any(feature = "socks5", feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) fn packet_path_carrier_descriptor(
    cache_key: String,
    server: &str,
    port: u16,
) -> PacketPathCarrierDescriptor {
    PacketPathCarrierDescriptor {
        cache_key,
        server: server.to_owned(),
        port,
    }
}

#[cfg(any(feature = "socks5", feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) trait PacketPathCarrierDescriptorBuild {
    fn into_parts(self) -> (String, String, u16);
}

#[cfg(any(feature = "socks5", feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) fn packet_path_carrier_descriptor_from_build(
    build: impl PacketPathCarrierDescriptorBuild,
) -> PacketPathCarrierDescriptor {
    let (cache_key, server, port) = build.into_parts();
    packet_path_carrier_descriptor(cache_key, &server, port)
}

/// Datagram source params for a relay-chain final hop over a packet path.
///
/// Produced by `PreparedUdpPacketPathOperation::into_datagram_source`. The `cache_key`
/// feeds packet-path cache identity without exposing raw config parsing to the
/// manager.
#[derive(Clone)]
pub(crate) struct UdpDatagramDescriptor {
    pub(crate) tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
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
impl UdpDatagramDescriptor {
    pub(crate) fn key_part(&self) -> UdpDatagramKey {
        UdpDatagramKey {
            tag: self.tag.clone(),
            server: self.server.clone(),
            port: self.port,
            cache_key: self.cache_key.clone(),
        }
    }

    pub(crate) fn endpoint(&self) -> UdpDatagramEndpoint {
        UdpDatagramEndpoint {
            server: self.server.clone(),
            port: self.port,
        }
    }
}

/// Adapter-provided datagram role output for packet-path relay chains.
///
/// The descriptor is the generic chain-management surface. The codec is the
/// protocol-provided packet framing object for the selected datagram hop.
#[derive(Clone)]
pub(crate) struct UdpDatagramSource {
    pub(crate) descriptor: UdpDatagramDescriptor,
    pub(crate) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn udp_datagram_source(
    tag: &str,
    server: &str,
    port: u16,
    cache_key: String,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> UdpDatagramSource {
    UdpDatagramSource {
        descriptor: UdpDatagramDescriptor {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            cache_key,
        },
        codec,
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) trait UdpDatagramSourceBuild {
    fn into_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    );
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn udp_datagram_source_from_build(
    build: impl UdpDatagramSourceBuild,
) -> UdpDatagramSource {
    let (tag, server, port, cache_key, codec) = build.into_parts();
    udp_datagram_source(&tag, &server, port, cache_key, codec)
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
impl UdpDatagramSource {
    pub(crate) fn descriptor(&self) -> &UdpDatagramDescriptor {
        &self.descriptor
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UdpDatagramKey {
    pub(crate) tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UdpDatagramEndpoint {
    server: String,
    port: u16,
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
impl UdpDatagramEndpoint {
    pub(crate) fn target(&self) -> Address {
        Address::Domain(self.server.clone())
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn upstream(&self) -> (String, u16) {
        (self.server.clone(), self.port)
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathLookupKey {
    carrier_cache_key: String,
    datagram: UdpDatagramKey,
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

    pub(crate) fn into_path_parts(self) -> (String, UdpDatagramKey) {
        (self.carrier_cache_key, self.datagram)
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathFlowSnapshot {
    carrier_cache_key: String,
    datagram: UdpDatagramKey,
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
impl PacketPathFlowSnapshot {
    fn from_parts(datagram: &UdpDatagramDescriptor, carrier: &PacketPathCarrierDescriptor) -> Self {
        Self {
            carrier_cache_key: carrier.cache_key.clone(),
            datagram: datagram.key_part(),
        }
    }

    pub(crate) fn lookup_key(&self) -> PacketPathLookupKey {
        PacketPathLookupKey {
            carrier_cache_key: self.carrier_cache_key.clone(),
            datagram: self.datagram.clone(),
        }
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
pub(crate) struct PacketPathFlowBinding {
    datagram: UdpDatagramSource,
    flow_snapshot: PacketPathFlowSnapshot,
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
impl PacketPathFlowBinding {
    pub(crate) fn new(
        datagram: UdpDatagramSource,
        carrier_desc: &PacketPathCarrierDescriptor,
    ) -> Self {
        let flow_snapshot = PacketPathFlowSnapshot::from_parts(datagram.descriptor(), carrier_desc);
        Self {
            datagram,
            flow_snapshot,
        }
    }

    pub(crate) fn into_parts(self) -> (UdpDatagramSource, PacketPathFlowSnapshot) {
        (self.datagram, self.flow_snapshot)
    }
}
