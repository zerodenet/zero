//! Packet path chain abstractions for UDP relay chains.
//!
//! Re-exports [`UdpPacketPath`] and [`DatagramCodec`] from `zero-traits`.
//! Protocol crates implement the `zero-traits` versions directly; the proxy
//! runtime uses these re-exports with [`Address`] from `zero-core`.

use std::vec::Vec;

use zero_core::Address;
use zero_engine::EngineError;

/// Packet path transport for relaying raw UDP payloads.
pub(in crate::runtime) use zero_traits::UdpPacketPath;

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(in crate::runtime) use zero_traits::DatagramCodec;

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`super::UdpDispatch::poll_chain_response`].
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;
