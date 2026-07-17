#[cfg(feature = "udp-runtime")]
use std::vec::Vec;

#[cfg(feature = "udp-runtime")]
use tokio::task::JoinSet;
#[cfg(feature = "udp-runtime")]
use zero_core::Address;
#[cfg(feature = "udp-runtime")]
use zero_engine::EngineError;

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via UDP dispatch chain polling.
#[cfg(feature = "udp-runtime")]

pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

/// Runtime context shared by UDP outbound managers for one send operation.
#[cfg(feature = "udp-runtime")]

pub(crate) struct UdpFlowContext<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
}

/// Borrowed target payload for one UDP send operation.
#[cfg(feature = "udp-runtime")]
#[derive(Clone, Copy)]
pub(crate) struct UdpPacketRef<'a> {
    pub(crate) target: &'a Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
}
