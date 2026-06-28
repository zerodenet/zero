use std::any::Any;
use std::marker::PhantomData;

use async_trait::async_trait;
use zero_engine::EngineError;

use super::cache::{ManagedDatagramConnectionCache, ManagedUdpConnectionCache};
use super::connection::{SharedManagedDatagramUdpConnection, SharedManagedUdpConnection};
use super::flow::ManagedUdpFlowResume;
use super::model::{ManagedDatagramFlowHandler, ManagedExistingSend};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;

#[async_trait]
pub(crate) trait ManagedDatagramFlowConnector<T>: Send + Sync {
    const INITIAL_PACKET_PRE_SENT: bool;

    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint<'_>,
    ) -> ManagedDatagramConnectorFlow;

    async fn establish(
        &self,
        proxy: Option<&Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;
}

pub(crate) struct ManagedDatagramConnectorFlow {
    cache_key: String,
}

impl ManagedDatagramConnectorFlow {
    pub(crate) fn new(cache_key: String) -> Self {
        Self { cache_key }
    }

    fn cache_key(self) -> String {
        self.cache_key
    }
}

pub(crate) struct ManagedDatagramFlowManager<T, C> {
    upstreams: ManagedUdpConnectionCache,
    connector: C,
    establish_stage: &'static str,
    mismatch_stage: &'static str,
    mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

pub(crate) struct ManagedDatagramSocketFlowManager<T, C> {
    upstreams: ManagedDatagramConnectionCache,
    connector: C,
    establish_stage: &'static str,
    send_stage: &'static str,
    mismatch_stage: &'static str,
    mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

impl<T, C> ManagedDatagramFlowManager<T, C> {
    pub(crate) fn new(
        connector: C,
        establish_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
            connector,
            establish_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}

impl<T, C> ManagedDatagramSocketFlowManager<T, C> {
    pub(crate) fn new(
        connector: C,
        establish_stage: &'static str,
        send_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedDatagramConnectionCache::new(),
            connector,
            establish_stage,
            send_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}

impl<T, C> ManagedDatagramFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: Option<&Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let cache_key = self.connector.connector_flow(&resume, endpoint).cache_key();
        let establish = self
            .connector
            .establish(proxy, endpoint, resume, packet_ref);
        let result = if C::INITIAL_PACKET_PRE_SENT {
            self.upstreams
                .send_or_insert_pre_sent_key(
                    cache_key,
                    ctx.chain_tasks,
                    ctx.session_id,
                    packet_ref,
                    establish,
                )
                .await
        } else {
            self.upstreams
                .send_or_insert_key(
                    cache_key,
                    ctx.chain_tasks,
                    ctx.session_id,
                    packet_ref,
                    establish,
                )
                .await
        };
        result.map_err(|e| FlowFailure {
            stage: self.establish_stage,
            error: e,
            upstream: Some(endpoint.upstream()),
        })
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}

#[async_trait]
pub(crate) trait ManagedDatagramSocketFlowConnector<T>: Send + Sync {
    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint<'_>,
    ) -> ManagedDatagramSocketConnectorFlow;

    async fn establish(
        &self,
        proxy: Option<&Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError>;
}

pub(crate) struct ManagedDatagramSocketConnectorFlow {
    cache_key: String,
}

impl ManagedDatagramSocketConnectorFlow {
    pub(crate) fn new(cache_key: String) -> Self {
        Self { cache_key }
    }

    fn cache_key(self) -> String {
        self.cache_key
    }
}

impl<T, C> ManagedDatagramSocketFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramSocketFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: Option<&Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let cache_key = self.connector.connector_flow(&resume, endpoint).cache_key();
        let connection = self
            .upstreams
            .get_or_insert_key(
                cache_key,
                self.connector
                    .establish(proxy, endpoint, resume, packet_ref),
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.establish_stage,
                error,
                upstream: Some(endpoint.upstream()),
            })?;

        connection
            .send_datagram(
                ctx.chain_tasks,
                ctx.session_id,
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.send_stage,
                error,
                upstream: Some(endpoint.upstream()),
            })
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}

#[async_trait]
impl<T, C> ManagedDatagramFlowHandler for ManagedDatagramSocketFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramSocketFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedDatagramSocketFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedDatagramSocketFlowManager::send_managed_existing(self, request).await
    }
}

#[async_trait]
impl<T, C> ManagedDatagramFlowHandler for ManagedDatagramFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedDatagramFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedDatagramFlowManager::send_managed_existing(self, request).await
    }
}

fn managed_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
