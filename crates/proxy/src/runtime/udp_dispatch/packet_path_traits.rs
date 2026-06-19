//! Packet path chain abstractions for UDP relay chains.
//!
//! Re-exports [`UdpPacketPath`] and [`DatagramCodec`] from `zero-traits`.
//! Protocol crates implement the `zero-traits` versions directly; the proxy
//! runtime uses these re-exports with [`Address`] from `zero-core`.

use std::vec::Vec;

use async_trait::async_trait;
use zero_core::Address;
use zero_engine::EngineError;

use tokio::task::JoinSet;

use crate::runtime::orchestration::OutboundEndpoint;

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(in crate::runtime) use zero_traits::DatagramCodec;

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

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`super::UdpDispatch::poll_chain_response`].
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

/// Carrier identity for cache lookup (cheap, computed before dialing).
///
/// Produced by `ProtocolAdapter::udp_packet_path_carrier_descriptor`. The
/// `cache_key` uniquely identifies one carrier connection so the manager can
/// reuse it across packets; `server`/`port` are the endpoint for diagnostics.
pub(crate) struct PacketPathCarrierDescriptor {
    pub(crate) cache_key: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

/// Datagram source params for a relay-chain final hop over a packet path.
///
/// Produced by `ProtocolAdapter::udp_datagram_source`. The manager builds the
/// inner `DatagramCodec` from `cipher` + `password`; `tag`/`server`/`port`
/// feed the outbound result + cache key.
pub(crate) struct UdpDatagramSource<'a> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) cipher: &'a str,
}

/// Runtime context shared by UDP outbound managers for one send operation.
pub(crate) struct UdpFlowContext<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
}

/// Borrowed target payload for one UDP send operation.
pub(crate) struct UdpPacketRef<'a> {
    pub(crate) target: &'a Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
}

pub(crate) type UdpPeerEndpoint<'a> = OutboundEndpoint<'a>;

/// Shadowsocks UDP peer parameters.
pub(crate) struct SsUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) cipher: &'a str,
}

/// Hysteria2 UDP peer parameters.
pub(crate) struct H2UdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) client_fingerprint: Option<&'a str>,
}

/// Trojan UDP peer parameters.
pub(crate) struct TrojanUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) relay_chain: bool,
}

/// Mieru UDP peer parameters.
pub(crate) struct MieruUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) relay_chain: bool,
}
