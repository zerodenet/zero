use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::connection::{ManagedDatagramUdpConnection, SharedManagedDatagramUdpConnection};
use super::flow::{ManagedDatagramFlow, ManagedUdpFlowSnapshot};
use super::model::{ManagedDatagramFlowHandler, ManagedExistingSend};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

pub(crate) type ManagedDatagramResponse = (Address, u16, Vec<u8>);

struct ManagedDatagramResponseWaiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<ManagedDatagramResponse>,
}

pub(crate) struct ManagedDatagramResponseWaiters {
    waiters: Arc<Mutex<VecDeque<ManagedDatagramResponseWaiter>>>,
}

impl ManagedDatagramResponseWaiters {
    pub(crate) fn new() -> Self {
        Self {
            waiters: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(crate) fn clone_handle(&self) -> Self {
        Self {
            waiters: self.waiters.clone(),
        }
    }

    pub(crate) fn register(
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

    pub(crate) fn remove(&self, target: &Address, port: u16) -> bool {
        self.remove_waiter(target, port).is_some()
    }

    pub(crate) fn deliver(&self, target: Address, port: u16, payload: Vec<u8>) -> bool {
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

#[async_trait::async_trait]
pub(crate) trait ManagedDatagramSender: Send + Sync {
    async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError>;
}

struct ManagedDatagramConnection {
    sender: Arc<dyn ManagedDatagramSender>,
    waiters: ManagedDatagramResponseWaiters,
    closed_message: &'static str,
}

#[async_trait::async_trait]
impl ManagedDatagramUdpConnection for ManagedDatagramConnection {
    async fn send_datagram(
        &self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let response_rx = self.waiters.register(target, port);
        if let Err(error) = self.sender.send_datagram(target, port, payload).await {
            self.waiters.remove(target, port);
            return Err(error);
        }

        spawn_datagram_response_bridge(chain_tasks, response_rx, session_id, self.closed_message);
        Ok(payload.len())
    }
}

pub(crate) fn managed_datagram_connection(
    sender: Arc<dyn ManagedDatagramSender>,
    response_rx: tokio::sync::broadcast::Receiver<ManagedDatagramResponse>,
    closed_message: &'static str,
) -> SharedManagedDatagramUdpConnection {
    let waiters = ManagedDatagramResponseWaiters::new();
    spawn_upstream_response_pump(response_rx, waiters.clone_handle());
    Arc::new(ManagedDatagramConnection {
        sender,
        waiters,
        closed_message,
    })
}

pub(crate) fn spawn_datagram_response_bridge(
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

fn spawn_upstream_response_pump(
    mut response_rx: tokio::sync::broadcast::Receiver<ManagedDatagramResponse>,
    waiters: ManagedDatagramResponseWaiters,
) {
    tokio::spawn(async move {
        while let Ok((target, port, payload)) = response_rx.recv().await {
            waiters.deliver(target, port, payload);
        }
    });
}

pub(super) struct ManagedDatagramState {
    handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>,
}

impl ManagedDatagramState {
    pub(super) fn new(handlers: Vec<Box<dyn ManagedDatagramFlowHandler>>) -> Self {
        Self { handlers }
    }

    pub(super) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Option<Result<usize, FlowFailure>> {
        for handler in &mut self.handlers {
            if !handler.supports_managed_existing(&flow.resume) {
                continue;
            }
            return Some(
                handler
                    .send_managed_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                    .await,
            );
        }
        None
    }

    pub(super) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ManagedUdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
        let resume = snapshot.resume();
        for handler in &mut self.handlers {
            if !handler.supports_managed_existing(resume) {
                continue;
            }
            return Some(
                handler
                    .send_managed_existing(ManagedExistingSend::forwarded(
                        chain_tasks,
                        proxy,
                        flow,
                        resume.clone(),
                        upstream.server,
                        upstream.port,
                        payload,
                    ))
                    .await,
            );
        }
        None
    }
}
