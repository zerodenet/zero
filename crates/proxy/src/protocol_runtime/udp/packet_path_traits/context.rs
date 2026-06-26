use std::vec::Vec;

use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

/// A response item produced by a chain-outbound recv bridge task.
///
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via UDP dispatch chain polling.
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

#[derive(Clone)]
pub(crate) struct UdpResponsePacket {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
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
