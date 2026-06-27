use std::any::Any;
use std::marker::PhantomData;

use async_trait::async_trait;
use zero_engine::EngineError;

use super::cache::ManagedUdpConnectionCache;
use super::connection::SharedManagedUdpConnection;
use super::flow::ManagedUdpFlowResume;
use super::model::{ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[async_trait]
pub(crate) trait ManagedStreamFlowConnector<T>: Send + Sync {
    fn flow_cache_key(&self, resume: &T, endpoint: OutboundEndpoint<'_>, session_id: u64)
        -> String;

    fn requires_relay_upstream(&self, resume: &T) -> bool;

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
    ) -> Result<SharedManagedUdpConnection, EngineError>;

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        resume: T,
    ) -> Result<SharedManagedUdpConnection, EngineError>;
}

pub(crate) struct ManagedStreamFlowManager<T, C> {
    upstreams: ManagedUdpConnectionCache,
    connector: C,
    establish_stage: &'static str,
    relay_upstream_stage: &'static str,
    relay_establish_stage: &'static str,
    relay_send_stage: &'static str,
    mismatch_stage: &'static str,
    mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

impl<T, C> ManagedStreamFlowManager<T, C> {
    pub(crate) fn new(
        connector: C,
        establish_stage: &'static str,
        relay_upstream_stage: &'static str,
        relay_establish_stage: &'static str,
        relay_send_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
            connector,
            establish_stage,
            relay_upstream_stage,
            relay_establish_stage,
            relay_send_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}

impl<T, C> ManagedStreamFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedStreamFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        if self.connector.requires_relay_upstream(&resume) {
            return Err(FlowFailure {
                stage: self.relay_upstream_stage,
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "relay upstream is not established",
                )),
                upstream: Some(endpoint.upstream()),
            });
        }
        let cache_key = self.connector.flow_cache_key(&resume, endpoint, session_id);

        self.upstreams
            .send_or_insert_key(
                cache_key,
                ctx.chain_tasks,
                session_id,
                packet_ref,
                self.connector.establish_direct(proxy, endpoint, resume),
            )
            .await
            .map_err(|e| FlowFailure {
                stage: self.establish_stage,
                error: e,
                upstream: Some(endpoint.upstream()),
            })
    }

    async fn send_relay(
        &mut self,
        ctx: UdpFlowContext<'_>,
        stream: TcpRelayStream,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let cache_key = self.connector.flow_cache_key(&resume, endpoint, session_id);
        let entry = self
            .connector
            .establish_relay(stream, resume)
            .await
            .map_err(|e| FlowFailure {
                stage: self.relay_establish_stage,
                error: e,
                upstream: Some(endpoint.upstream()),
            })?;

        self.upstreams
            .insert_and_send_key(cache_key, ctx.chain_tasks, session_id, packet_ref, entry)
            .await
            .map_err(|error| FlowFailure {
                stage: self.relay_send_stage,
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
        let Some(proxy) = request.proxy else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                "expected proxy context for managed stream UDP flow",
            ));
        };
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            proxy,
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

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<T>() else {
            return Err(managed_mismatch(
                self.mismatch_stage,
                request.server,
                request.port,
                self.mismatch_message,
            ));
        };
        self.send_relay(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.stream,
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
impl<T, C> ManagedStreamFlowHandler for ManagedStreamFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedStreamFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedStreamFlowManager::supports_managed_existing(self, resume)
    }

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedStreamFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedStreamFlowManager::send_managed_existing(self, request).await
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedStreamFlowManager::send_managed_relay_existing(self, request).await
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
