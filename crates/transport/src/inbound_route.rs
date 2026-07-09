use core::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;

use zero_config::{FallbackConfig, InboundProtocolConfig};
use zero_core::{InboundMuxStreamRoute, InboundStreamRoute};
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;

use crate::ClientStream;
#[cfg(feature = "quic")]
use crate::{MeteredStream, RecordingStream};

pub struct InboundFallback<R> {
    pub config: FallbackConfig,
    pub replay: R,
}

pub type ReplayToUpstreamFuture<'a, S> =
    Pin<Box<dyn Future<Output = Result<S, io::Error>> + Send + 'a>>;

pub enum RouteAcceptResult<R, F> {
    Route(R),
    Fallback(InboundFallback<F>),
}

trait ReplayToUpstreamFn<S>: Send {
    fn call<'a>(self: Box<Self>, upstream: &'a mut TokioSocket) -> ReplayToUpstreamFuture<'a, S>;
}

impl<S, F> ReplayToUpstreamFn<S> for F
where
    F: Send + 'static + for<'a> FnOnce(&'a mut TokioSocket) -> ReplayToUpstreamFuture<'a, S>,
{
    fn call<'a>(self: Box<Self>, upstream: &'a mut TokioSocket) -> ReplayToUpstreamFuture<'a, S> {
        (*self)(upstream)
    }
}

pub struct OpaqueFallbackReplay<S> {
    replay: Option<Box<dyn ReplayToUpstreamFn<S>>>,
}

impl<S> OpaqueFallbackReplay<S> {
    pub fn new<F>(replay: F) -> Self
    where
        F: Send + 'static + for<'a> FnOnce(&'a mut TokioSocket) -> ReplayToUpstreamFuture<'a, S>,
    {
        Self {
            replay: Some(Box::new(replay)),
        }
    }
}

pub struct OpaqueStreamRoute<R> {
    inner: R,
}

impl<R> OpaqueStreamRoute<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<R> InboundStreamRoute for OpaqueStreamRoute<R>
where
    R: InboundStreamRoute + Send,
{
    type TcpStream = R::TcpStream;
    type UdpRelay = R::UdpRelay;

    async fn dispatch_inbound_route<E, FTcp, FTcpFut, FUdp, FUdpFut>(
        self,
        on_tcp: FTcp,
        on_udp: FUdp,
    ) -> Result<(), E>
    where
        FTcp: FnOnce(zero_core::Session, Self::TcpStream) -> FTcpFut + Send,
        FTcpFut: Future<Output = Result<(), E>> + Send,
        FUdp: FnOnce(zero_core::Session, Self::UdpRelay) -> FUdpFut + Send,
        FUdpFut: Future<Output = Result<(), E>> + Send,
    {
        self.inner.dispatch_inbound_route(on_tcp, on_udp).await
    }
}

pub struct OpaqueMuxRoute<R> {
    inner: R,
}

impl<R> OpaqueMuxRoute<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<R> InboundMuxStreamRoute for OpaqueMuxRoute<R>
where
    R: InboundMuxStreamRoute + Send,
{
    type TcpStream = R::TcpStream;
    type UdpRelay = R::UdpRelay;
    type MuxReader = R::MuxReader;
    type MuxServer = R::MuxServer;

    async fn dispatch_inbound_route<E, FTcp, FTcpFut, FUdp, FUdpFut, FMux, FMuxFut>(
        self,
        on_tcp: FTcp,
        on_udp: FUdp,
        on_mux: FMux,
    ) -> Result<(), E>
    where
        FTcp: FnOnce(zero_core::Session, Self::TcpStream) -> FTcpFut + Send,
        FTcpFut: Future<Output = Result<(), E>> + Send,
        FUdp: FnOnce(zero_core::Session, Self::UdpRelay) -> FUdpFut + Send,
        FUdpFut: Future<Output = Result<(), E>> + Send,
        FMux: FnOnce(Self::MuxReader, Self::MuxServer) -> FMuxFut + Send,
        FMuxFut: Future<Output = Result<(), E>> + Send,
    {
        self.inner
            .dispatch_inbound_route(on_tcp, on_udp, on_mux)
            .await
    }
}

#[async_trait::async_trait]
pub trait FallbackReplayToUpstream: Send {
    type Stream: ClientStream + Send + 'static;

    async fn replay_to_upstream(
        self,
        upstream: &mut TokioSocket,
    ) -> Result<Self::Stream, io::Error>;
}

#[async_trait::async_trait]
impl<S> FallbackReplayToUpstream for OpaqueFallbackReplay<S>
where
    S: ClientStream + Send + 'static,
{
    type Stream = S;

    async fn replay_to_upstream(
        mut self,
        upstream: &mut TokioSocket,
    ) -> Result<Self::Stream, io::Error> {
        let replay = self
            .replay
            .take()
            .expect("opaque fallback replay already consumed");
        replay.call(upstream).await
    }
}

#[async_trait::async_trait]
pub trait StreamRouteRequest: Send {
    type Route: Send + 'static;

    async fn accept_route(self, socket: TokioSocket) -> Result<Self::Route, EngineError>;
}

#[async_trait::async_trait]
pub trait MuxRouteRequest: Send {
    type Route: Send + 'static;

    async fn accept_route(self, socket: TokioSocket) -> Result<Self::Route, EngineError>;
}

pub trait ProtocolInboundRequestMetadata {
    const ERROR_PROTOCOL_NAME: &'static str;

    fn protocol_name(&self) -> &'static str;
}

pub trait ProtocolInboundRequestFactory: Sized {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError>;
}

pub enum TransportInboundBindTarget {
    Tcp,
    #[cfg(feature = "quic")]
    Quic(crate::quic::QuicInbound),
}

#[async_trait::async_trait]
pub trait ProtocolInboundBindPlan: Sized {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError>;

    async fn bind(&self, listen_addr: &str) -> Result<TransportInboundBindTarget, EngineError>;
}

pub trait ProtocolStreamRouteDispatchMetadata: ProtocolInboundRequestMetadata {
    const UDP_PROTOCOL: &'static str;
}

pub trait ProtocolMuxRouteDispatchMetadata: ProtocolInboundRequestMetadata {
    const UDP_PROTOCOL: &'static str;
    const MUX_PROTOCOL: &'static str;
    const PANIC_MESSAGE: &'static str;
    const ABORT_ON_END: bool;
    const READ_ERROR_LOG: &'static str;
}

pub trait RecordedProtocolMuxRouteDispatchMetadata: ProtocolInboundRequestMetadata {
    type ResponseProtocol: Clone + Send + Sync + 'static;

    const UDP_PROTOCOL: &'static str;
    const MUX_PROTOCOL: &'static str;
    const PANIC_MESSAGE: &'static str;
    const ABORT_ON_END: bool;

    fn response_protocol(&self) -> Self::ResponseProtocol;
}

#[cfg(feature = "quic")]
#[async_trait::async_trait]
pub trait RecordedBoundMuxRouteRequest:
    Clone
    + Send
    + Sync
    + 'static
    + ProtocolInboundRequestFactory
    + RecordedProtocolMuxRouteDispatchMetadata
{
    type TcpStream: ClientStream + 'static;
    type TcpRoute: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<Self::TcpStream>>,
            MuxReader = MeteredStream<RecordingStream<Self::TcpStream>>,
        > + Send
        + 'static;
    type TcpFallback: FallbackReplayToUpstream + 'static;
    type QuicStream: ClientStream + 'static;
    type QuicRoute: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<Self::QuicStream>>,
            MuxReader = MeteredStream<RecordingStream<Self::QuicStream>>,
        > + Send
        + 'static;
    type QuicFallback: FallbackReplayToUpstream + 'static;

    async fn accept_tcp_bound_route(
        self,
        socket: TokioSocket,
    ) -> Result<Option<RouteAcceptResult<Self::TcpRoute, Self::TcpFallback>>, EngineError>;

    async fn accept_quic_bound_route(
        self,
        stream: crate::quic::QuicStream,
    ) -> Result<RouteAcceptResult<Self::QuicRoute, Self::QuicFallback>, EngineError>;
}
