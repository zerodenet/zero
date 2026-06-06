//! Packet path chain abstractions for UDP relay chains.
//!
//! These traits define the "previous hop packet path carries next hop
//! datagram" model. Adding new combinations requires implementing
//! [`UdpPacketPath`] and [`DatagramCodec`], not creating protocol-pair modules.

use std::vec::Vec;

use zero_core::Address;
use zero_engine::EngineError;

/// A packet-oriented transport that carries raw UDP payloads.
///
/// Models a carrier that provides send/recv for raw datagrams.
/// Implementations handle their own transport framing (e.g. SOCKS5 UDP
/// header); callers provide and receive plain payloads only.
#[allow(async_fn_in_trait)]
pub(in crate::runtime) trait UdpPacketPath: Send + Sync + 'static {
    /// Send `payload` to `target:port` through this transport.
    async fn send_to(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<(), EngineError>;

    /// Receive the next datagram, stripping transport framing.
    ///
    /// Returns the number of inner payload bytes written to `buf`.
    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

/// Encode/decode UDP datagrams for the inner protocol of a relay chain.
///
/// Each protocol that can be the final hop of a datagram-over-packet-path
/// chain implements this. The codec captures protocol-specific parameters
/// (cipher, password, etc.) so the manager stays protocol-agnostic.
pub(in crate::runtime) trait DatagramCodec: Send + Sync + 'static {
    fn encode(&self, target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, EngineError>;
    fn decode(&self, data: &[u8]) -> Option<(Address, u16, Vec<u8>)>;
}

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`super::UdpDispatch::poll_chain_response`].
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;
