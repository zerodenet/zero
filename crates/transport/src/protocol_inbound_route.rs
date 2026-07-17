use core::future::Future;
use std::io;
use std::pin::Pin;

use zero_core::{InboundMuxStreamRoute, InboundStreamRoute};
use zero_platform_tokio::TokioSocket;

use crate::{ClientStream, OwnedInboundFallbackProfile};

pub struct InboundFallback<R> {
    pub config: OwnedInboundFallbackProfile,
    pub replay: R,
}

pub type ReplayToUpstreamFuture<'a, S> =
    Pin<Box<dyn Future<Output = Result<S, io::Error>> + Send + 'a>>;

pub enum RouteAcceptResult<R, F> {
    Route(R),
    Fallback(InboundFallback<F>),
}

#[derive(Clone, Copy)]
pub struct RecordedMuxRouteDefaults {
    pub udp_protocol: &'static str,
    pub mux_protocol: &'static str,
    pub panic_message: &'static str,
    pub abort_on_end: bool,
    pub udp_accept_log_message: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub struct NoClientMuxRouteDefaults {
    pub udp_protocol: &'static str,
    pub mux_protocol: &'static str,
    pub panic_message: &'static str,
    pub abort_on_end: bool,
    pub read_error_log: &'static str,
}

#[derive(Clone, Copy)]
pub struct NoClientStreamRouteDefaults {
    pub udp_protocol: &'static str,
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
