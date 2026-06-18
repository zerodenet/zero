//! Packet path chain abstractions for UDP relay chains.
//!
//! Re-exports [`UdpPacketPath`] and [`DatagramCodec`] from `zero-traits`.
//! Protocol crates implement the `zero-traits` versions directly; the proxy
//! runtime uses these re-exports with [`Address`] from `zero-core`.

use std::vec::Vec;

use zero_core::Address;
use zero_engine::EngineError;

use tokio::task::JoinSet;

use crate::runtime::orchestration::OutboundEndpoint;

/// Packet path transport for relaying raw UDP payloads.
pub(in crate::runtime) use zero_traits::UdpPacketPath;

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(in crate::runtime) use zero_traits::DatagramCodec;

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`super::UdpDispatch::poll_chain_response`].
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

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
