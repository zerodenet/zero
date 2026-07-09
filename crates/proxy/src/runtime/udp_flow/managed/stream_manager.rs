use std::any::Any;
use std::marker::PhantomData;

use async_trait::async_trait;
use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::managed_udp::{
    ManagedPacketUdpResume, ManagedTupleUdpResume, ProtocolManagedPacketUdpFlowResumeConnectionOps,
    ProtocolManagedStreamConnectorParts, ProtocolManagedTupleUdpFlowResumeConnectionOps,
};

use super::cache::ManagedUdpConnectionCache;
use super::connection::{
    managed_packet_udp_connection_from_flow, managed_tuple_udp_connection_from_flow,
    SharedManagedUdpConnection,
};
use super::flow::ManagedUdpFlowResume;
use super::model::{ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[async_trait]
pub(crate) trait ManagedStreamFlowConnector:
    Any + Clone + Send + Sync + std::fmt::Debug + 'static
{
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow;

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: Option<&Proxy>,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;
}

pub(crate) struct ManagedStreamConnectorFlow {
    cache_key: String,
    requires_relay_upstream: bool,
}

impl ManagedStreamConnectorFlow {
    pub(crate) fn new(cache_key: String, requires_relay_upstream: bool) -> Self {
        Self {
            cache_key,
            requires_relay_upstream,
        }
    }

    fn into_parts(self) -> (String, bool) {
        (self.cache_key, self.requires_relay_upstream)
    }
}

pub(crate) trait ManagedStreamConnectorFlowBuild {
    fn into_parts(self) -> (String, bool);
}

impl<T> ManagedStreamConnectorFlowBuild for T
where
    T: ProtocolManagedStreamConnectorParts,
{
    fn into_parts(self) -> (String, bool) {
        self.into_managed_connector_parts()
    }
}

pub(crate) fn managed_stream_connector_flow_from_build(
    build: impl ManagedStreamConnectorFlowBuild,
) -> ManagedStreamConnectorFlow {
    let (cache_key, requires_relay_upstream) = build.into_parts();
    ManagedStreamConnectorFlow::new(cache_key, requires_relay_upstream)
}

#[async_trait]
impl<T> ManagedStreamFlowConnector for ManagedTupleUdpResume<T>
where
    T: ProtocolManagedTupleUdpFlowResumeConnectionOps + Clone,
{
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        managed_stream_connector_flow_from_build(self.0.connector_flow_for_resume(
            endpoint.server,
            endpoint.port,
            session_id,
        ))
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_direct_protocol_connection(session, move |server, port| {
                proxy.connect_upstream_host_owned(server.to_owned(), port)
            })
            .await?;
        Ok(managed_tuple_udp_connection_from_flow(connection))
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_relay_protocol_connection(stream, session, tls_server_name)
            .await?;
        Ok(managed_tuple_udp_connection_from_flow(connection))
    }
}

#[async_trait]
impl<T> ManagedStreamFlowConnector for ManagedPacketUdpResume<T>
where
    T: ProtocolManagedPacketUdpFlowResumeConnectionOps + Clone,
{
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        managed_stream_connector_flow_from_build(self.0.connector_flow_for_resume(
            endpoint.server,
            endpoint.port,
            session_id,
        ))
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_direct_protocol_connection(session, move |server, port| {
                proxy.connect_upstream_host_owned(server.to_owned(), port)
            })
            .await?;
        Ok(managed_packet_udp_connection_from_flow(connection))
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_relay_protocol_connection(stream, session, tls_server_name)
            .await?;
        Ok(managed_packet_udp_connection_from_flow(connection))
    }
}

pub(crate) struct ManagedStreamFlowManager<T> {
    upstreams: ManagedUdpConnectionCache,
    establish_stage: &'static str,
    relay_upstream_stage: &'static str,
    relay_establish_stage: &'static str,
    relay_send_stage: &'static str,
    mismatch_stage: &'static str,
    mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

struct ManagedStreamRelayRequest<'a, T> {
    ctx: UdpFlowContext<'a>,
    stream: TcpRelayStream,
    tls_server_name: Option<&'a str>,
    proxy: Option<&'a Proxy>,
    session: &'a Session,
    endpoint: OutboundEndpoint<'a>,
    resume: T,
    packet_ref: UdpPacketRef<'a>,
}

impl<T> ManagedStreamFlowManager<T> {
    pub(crate) fn new(
        establish_stage: &'static str,
        relay_upstream_stage: &'static str,
        relay_establish_stage: &'static str,
        relay_send_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
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

impl<T> ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: T,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let connector_flow = resume.connector_flow(endpoint, session_id);
        let (cache_key, requires_relay_upstream) = connector_flow.into_parts();
        if requires_relay_upstream {
            if let Some(sent) = self
                .upstreams
                .send_existing_key(cache_key, ctx.chain_tasks, session_id, packet_ref)
                .await
                .map_err(|error| FlowFailure {
                    stage: self.relay_send_stage,
                    error,
                    upstream: Some(endpoint.upstream()),
                })?
            {
                return Ok(sent);
            }
            return Err(FlowFailure {
                stage: self.relay_upstream_stage,
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "relay upstream is not established",
                )),
                upstream: Some(endpoint.upstream()),
            });
        }

        self.upstreams
            .send_or_insert_key(
                cache_key,
                ctx.chain_tasks,
                session_id,
                packet_ref,
                resume.establish_direct(proxy, session, endpoint),
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
        request: ManagedStreamRelayRequest<'_, T>,
    ) -> Result<usize, FlowFailure> {
        let session_id = request.ctx.session_id;
        let upstream = request.endpoint.upstream();
        let (cache_key, _) = request
            .resume
            .connector_flow(request.endpoint, session_id)
            .into_parts();
        let entry = request
            .resume
            .establish_relay(
                request.stream,
                request.tls_server_name,
                request.proxy,
                request.session,
                request.endpoint,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: self.relay_establish_stage,
                error: e,
                upstream: Some(upstream.clone()),
            })?;

        self.upstreams
            .insert_and_send_key(
                cache_key,
                request.ctx.chain_tasks,
                session_id,
                request.packet_ref,
                entry,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: self.relay_send_stage,
                error,
                upstream: Some(upstream),
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
            request.session,
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
        self.send_relay(ManagedStreamRelayRequest {
            ctx: UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            stream: request.stream,
            tls_server_name: request.tls_server_name,
            proxy: request.proxy,
            session: request.session,
            endpoint: OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            resume,
            packet_ref: UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        })
        .await
    }
}

#[async_trait]
impl<T> ManagedStreamFlowHandler for ManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
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
