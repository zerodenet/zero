use crate::runtime::udp_flow::packet_path::ChainTask;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

pub(in crate::runtime::udp_flow::managed::datagram) type ManagedDatagramResponse =
    (Address, u16, Vec<u8>);

struct ManagedDatagramResponseWaiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<ManagedDatagramResponse>,
}

pub(in crate::runtime::udp_flow::managed::datagram) struct ManagedDatagramResponseWaiters {
    waiters: Arc<Mutex<VecDeque<ManagedDatagramResponseWaiter>>>,
}

impl ManagedDatagramResponseWaiters {
    pub(in crate::runtime::udp_flow::managed::datagram) fn new() -> Self {
        Self {
            waiters: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(in crate::runtime::udp_flow::managed::datagram) fn clone_handle(&self) -> Self {
        Self {
            waiters: self.waiters.clone(),
        }
    }

    pub(in crate::runtime::udp_flow::managed::datagram) fn register(
        &self,
        target: &Address,
        port: u16,
    ) -> oneshot::Receiver<ManagedDatagramResponse> {
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .expect("managed datagram waiters lock poisoned")
            .push_back(ManagedDatagramResponseWaiter {
                target: target.clone(),
                port,
                tx,
            });
        rx
    }

    pub(in crate::runtime::udp_flow::managed::datagram) fn remove(
        &self,
        target: &Address,
        port: u16,
    ) -> bool {
        self.remove_waiter(target, port).is_some()
    }

    pub(in crate::runtime::udp_flow::managed::datagram) fn deliver(
        &self,
        target: Address,
        port: u16,
        payload: Vec<u8>,
    ) -> bool {
        let Some(waiter) = self.remove_waiter(&target, port) else {
            return false;
        };
        waiter.tx.send((target, port, payload)).is_ok()
    }

    fn remove_waiter(&self, target: &Address, port: u16) -> Option<ManagedDatagramResponseWaiter> {
        let mut waiters = self
            .waiters
            .lock()
            .expect("managed datagram waiters lock poisoned");
        let index = waiters
            .iter()
            .position(|waiter| waiter.target == *target && waiter.port == port)?;
        waiters.remove(index)
    }
}

pub(in crate::runtime::udp_flow::managed::datagram) fn spawn_datagram_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    response_rx: oneshot::Receiver<ManagedDatagramResponse>,
    session_id: u64,
    closed_message: &'static str,
) {
    chain_tasks.spawn(async move {
        match response_rx.await {
            Ok((resp_target, resp_port, resp_payload)) => {
                Ok((resp_target, resp_port, resp_payload, Some(session_id)))
            }
            Err(_) => Err(EngineError::Io(std::io::Error::other(closed_message))),
        }
    });
}

pub(in crate::runtime::udp_flow::managed::datagram) fn spawn_upstream_response_pump(
    mut response_rx: tokio::sync::broadcast::Receiver<ManagedDatagramResponse>,
    waiters: ManagedDatagramResponseWaiters,
) {
    tokio::spawn(async move {
        while let Ok((target, port, payload)) = response_rx.recv().await {
            waiters.deliver(target, port, payload);
        }
    });
}
