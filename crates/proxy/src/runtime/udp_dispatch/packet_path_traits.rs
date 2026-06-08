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
pub(super) struct UdpFlowContext<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
}

/// Borrowed target payload for one UDP send operation.
pub(super) struct UdpPacketRef<'a> {
    pub(super) target: &'a Address,
    pub(super) port: u16,
    pub(super) payload: &'a [u8],
}

pub(super) type UdpPeerEndpoint<'a> = OutboundEndpoint<'a>;

/// Shadowsocks UDP peer parameters.
pub(super) struct SsUdpPeer<'a> {
    pub(super) endpoint: UdpPeerEndpoint<'a>,
    pub(super) password: &'a str,
    pub(super) cipher: &'a str,
}

/// Hysteria2 UDP peer parameters.
pub(super) struct H2UdpPeer<'a> {
    pub(super) endpoint: UdpPeerEndpoint<'a>,
    pub(super) password: &'a str,
    pub(super) client_fingerprint: Option<&'a str>,
}

/// Trojan UDP peer parameters.
pub(super) struct TrojanUdpPeer<'a> {
    pub(super) endpoint: UdpPeerEndpoint<'a>,
    pub(super) password: &'a str,
    pub(super) sni: Option<&'a str>,
    pub(super) insecure: bool,
    pub(super) client_fingerprint: Option<&'a str>,
    pub(super) relay_chain: bool,
}

/// Mieru UDP peer parameters.
pub(super) struct MieruUdpPeer<'a> {
    pub(super) endpoint: UdpPeerEndpoint<'a>,
    pub(super) username: &'a str,
    pub(super) password: &'a str,
    pub(super) relay_chain: bool,
}
