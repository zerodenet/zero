use std::any::Any;
use std::sync::Arc;

use tokio::task::JoinSet;
use zero_core::Address;

use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use zero_core::Session;
use zero_engine::EngineError;

mod cache;
mod datagram;
pub(crate) mod model;
pub(crate) mod state;
mod stream;
mod stream_sender;

pub(crate) use cache::{
    ManagedDatagramConnectionCache, ManagedDatagramConnectionCacheKey, ManagedStreamConnection,
    ManagedStreamConnectionCache, ManagedStreamConnectionCacheKey, ManagedUdpConnectionCache,
    ManagedUdpConnectionCacheKey,
};
pub(crate) use model::{
    ManagedDatagramFlowHandler, ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler,
};
pub(crate) use state::{ManagedProtocolUdpState, ManagedUdpHandlers};
pub(crate) use stream_sender::ManagedStreamFlowSender;

#[async_trait::async_trait]
pub(crate) trait ManagedUdpConnection: Send + Sync {
    async fn send(&self, target: &Address, port: u16, payload: &[u8])
        -> Result<usize, EngineError>;

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64);
}

pub(crate) type SharedManagedUdpConnection = Arc<dyn ManagedUdpConnection>;

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

pub(crate) struct ManagedDatagramFlow<'a> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedStreamPacketFlow<'a> {
    pub(crate) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedRelayStreamFlow<'a> {
    pub(crate) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedUdpFlowRequest<'a> {
    pub(crate) chain_tasks:
        Option<&'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>>,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) kind: ManagedUdpFlowKind,
    pub(crate) outbound_tag: Option<&'a str>,
    pub(crate) session: &'a Session,
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedUdpFlowKind {
    Datagram,
    StreamPacket,
    RelayStream,
}

#[derive(Debug, Clone)]
pub(crate) enum ManagedUdpFlowSnapshot {
    Managed { resume: ManagedUdpFlowResume },
}

trait ManagedUdpFlowResumeObject: Any + Send + Sync + std::fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}

impl<T> ManagedUdpFlowResumeObject for T
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedUdpFlowResume {
    inner: Arc<dyn ManagedUdpFlowResumeObject>,
}

impl ManagedUdpFlowResume {
    pub(crate) fn new<T>(resume: T) -> Self
    where
        T: Any + Send + Sync + std::fmt::Debug,
    {
        Self {
            inner: Arc::new(resume),
        }
    }

    pub(crate) fn as_ref<T>(&self) -> Option<&T>
    where
        T: Any,
    {
        self.inner.as_any().downcast_ref::<T>()
    }

    pub(crate) fn cloned<T>(self) -> Option<T>
    where
        T: Any + Clone,
    {
        self.as_ref::<T>().cloned()
    }
}

impl ManagedUdpFlowSnapshot {
    pub(crate) fn managed(resume: ManagedUdpFlowResume) -> Self {
        Self::Managed { resume }
    }

    pub(crate) fn resume(&self) -> &ManagedUdpFlowResume {
        match self {
            Self::Managed { resume } => resume,
        }
    }
}
