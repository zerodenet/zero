use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::ShadowsocksUdpSocketFlow;

use crate::runtime::udp_flow::packet_path::ChainTask;

type SsRecvItem = (Address, u16, Vec<u8>);

struct SsResponseWaiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<SsRecvItem>,
}

pub(super) struct BridgeWaiters {
    waiters: Arc<Mutex<VecDeque<SsResponseWaiter>>>,
}

impl BridgeWaiters {
    pub(super) fn new() -> Self {
        Self {
            waiters: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(super) fn clone_handle(&self) -> Self {
        Self {
            waiters: self.waiters.clone(),
        }
    }

    pub(super) fn register(&self, target: &Address, port: u16) -> oneshot::Receiver<SsRecvItem> {
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .expect("ss waiters lock poisoned")
            .push_back(SsResponseWaiter {
                target: target.clone(),
                port,
                tx,
            });
        rx
    }

    pub(super) fn remove(&self, target: &Address, port: u16) -> bool {
        self.remove_waiter(target, port).is_some()
    }

    pub(super) fn deliver(&self, target: Address, port: u16, payload: Vec<u8>) -> bool {
        let Some(waiter) = self.remove_waiter(&target, port) else {
            return false;
        };
        waiter.tx.send((target, port, payload)).is_ok()
    }

    fn remove_waiter(&self, target: &Address, port: u16) -> Option<SsResponseWaiter> {
        let mut waiters = self.waiters.lock().expect("ss waiters lock poisoned");
        let index = waiters
            .iter()
            .position(|waiter| waiter.target == *target && waiter.port == port)?;
        waiters.remove(index)
    }
}

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    response_rx: oneshot::Receiver<SsRecvItem>,
    session_id: u64,
) {
    chain_tasks.spawn(async move {
        match response_rx.await {
            Ok((resp_target, resp_port, resp_payload)) => {
                Ok((resp_target, resp_port, resp_payload, Some(session_id)))
            }
            Err(_) => Err(EngineError::Io(std::io::Error::other("ss upstream closed"))),
        }
    });
}

pub(super) fn spawn_upstream_response_pump(
    flow: Arc<ShadowsocksUdpSocketFlow>,
    waiters: BridgeWaiters,
) {
    tokio::spawn(async move {
        let mut recv_rx = flow.subscribe();
        while let Ok((target, port, payload)) = recv_rx.recv().await {
            waiters.deliver(target, port, payload);
        }
    });
}
