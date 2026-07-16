use std::collections::VecDeque;
use std::sync::Mutex;

use tokio::sync::oneshot;
use zero_core::Address;

pub(in crate::runtime::udp_flow::packet_path_chain) type RecvItem = (Address, u16, Vec<u8>);

pub(in crate::runtime::udp_flow::packet_path_chain) struct Waiter {
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) tx: oneshot::Sender<RecvItem>,
}

pub(super) fn remove_waiter(
    waiters: &Mutex<VecDeque<Waiter>>,
    target: &Address,
    port: u16,
) -> Option<Waiter> {
    let mut waiters = waiters.lock().expect("packet path waiters lock poisoned");
    let index = waiters
        .iter()
        .position(|waiter| waiter.target == *target && waiter.port == port)?;
    waiters.remove(index)
}
