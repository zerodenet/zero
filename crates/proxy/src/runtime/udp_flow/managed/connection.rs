use std::sync::Arc;

use tokio::task::JoinSet;
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::ChainTask;

#[async_trait::async_trait]
pub(crate) trait ManagedUdpConnection: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64);
}

pub(crate) type SharedManagedUdpConnection = Arc<dyn ManagedUdpConnection>;

#[async_trait::async_trait]
pub(crate) trait ManagedTupleUdpSender: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message(&self) -> &'static str;
}

struct ManagedTupleUdpConnection {
    sender: Arc<dyn ManagedTupleUdpSender>,
}

#[async_trait::async_trait]
impl ManagedUdpConnection for ManagedTupleUdpConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.sender.send(target, port, payload).await
    }

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64) {
        spawn_tuple_response_bridge(
            chain_tasks,
            self.sender.subscribe_responses(),
            session_id,
            self.sender.closed_message(),
        );
    }
}

pub(crate) fn managed_tuple_udp_connection(
    sender: Arc<dyn ManagedTupleUdpSender>,
) -> SharedManagedUdpConnection {
    Arc::new(ManagedTupleUdpConnection { sender })
}

#[async_trait::async_trait]
pub(crate) trait ManagedPacketUdpSender: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn subscribe_responses(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket>;

    fn closed_message(&self) -> &'static str;
}

struct ManagedPacketUdpConnection {
    sender: Arc<dyn ManagedPacketUdpSender>,
}

#[async_trait::async_trait]
impl ManagedUdpConnection for ManagedPacketUdpConnection {
    async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.sender.send(target, port, payload).await
    }

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64) {
        spawn_response_bridge(
            chain_tasks,
            self.sender.subscribe_responses(),
            session_id,
            self.sender.closed_message(),
            |packet| (packet.target, packet.port, packet.payload),
        );
    }
}

pub(crate) fn managed_packet_udp_connection(
    sender: Arc<dyn ManagedPacketUdpSender>,
) -> SharedManagedUdpConnection {
    Arc::new(ManagedPacketUdpConnection { sender })
}

#[async_trait::async_trait]
pub(crate) trait ManagedDatagramUdpConnection: Send + Sync {
    async fn send_datagram(
        &self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError>;
}

pub(crate) type SharedManagedDatagramUdpConnection = Arc<dyn ManagedDatagramUdpConnection>;

pub(crate) fn spawn_response_bridge<T, F>(
    chain_tasks: &mut JoinSet<ChainTask>,
    mut response_rx: tokio::sync::broadcast::Receiver<T>,
    session_id: u64,
    closed_message: &'static str,
    mut into_packet: F,
) where
    T: Clone + Send + 'static,
    F: FnMut(T) -> (Address, u16, Vec<u8>) + Send + 'static,
{
    chain_tasks.spawn(async move {
        let response = response_rx
            .recv()
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other(closed_message)))?;
        let (target, port, payload) = into_packet(response);
        Ok((target, port, payload, Some(session_id)))
    });
}

pub(crate) fn spawn_tuple_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    response_rx: tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>,
    session_id: u64,
    closed_message: &'static str,
) {
    spawn_response_bridge(
        chain_tasks,
        response_rx,
        session_id,
        closed_message,
        |packet| packet,
    );
}
