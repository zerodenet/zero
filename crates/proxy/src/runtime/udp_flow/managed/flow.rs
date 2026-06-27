use std::any::Any;
use std::sync::Arc;

use zero_core::Session;

use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(crate) struct ManagedDatagramFlow<'a> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedStreamPacketFlow<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<ChainTask>,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedRelayStreamFlow<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<ChainTask>,
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
    pub(crate) chain_tasks: Option<&'a mut tokio::task::JoinSet<ChainTask>>,
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
