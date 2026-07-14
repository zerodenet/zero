use core::future::Future;
use core::pin::Pin;

use crate::RuntimeError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

pub type TransportOpenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TcpRelayStream, RuntimeError>> + Send + 'a>>;

pub trait TcpStreamTransportPlan: Clone + Send + Sync + 'static {
    fn open_direct_stream<'a, OpenSocket, OpenSocketFut>(
        &'a self,
        open_socket: OpenSocket,
    ) -> TransportOpenFuture<'a>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send + 'a;

    fn open_relay_stream<'a>(&'a self, stream: TcpRelayStream) -> TransportOpenFuture<'a>;
}

pub fn direct_stream_opener<'a, TPlan, OpenSocket, OpenSocketFut>(
    transport: &'a TPlan,
    open_socket: OpenSocket,
) -> impl FnOnce() -> TransportOpenFuture<'a>
where
    TPlan: TcpStreamTransportPlan,
    OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send + 'a,
{
    move || transport.open_direct_stream(open_socket)
}

pub fn relay_stream_opener<'a, TPlan>(
    transport: &'a TPlan,
    stream: TcpRelayStream,
) -> impl FnOnce() -> TransportOpenFuture<'a>
where
    TPlan: TcpStreamTransportPlan,
{
    move || transport.open_relay_stream(stream)
}

pub fn relay_stream_mapper<'a, TPlan>(
    transport: &'a TPlan,
) -> impl FnOnce(TcpRelayStream) -> TransportOpenFuture<'a>
where
    TPlan: TcpStreamTransportPlan,
{
    move |stream| transport.open_relay_stream(stream)
}

pub trait ProfiledTcpStreamTransportPlan<TProfile>: Clone + Send + Sync + 'static
where
    TProfile: Send,
{
    fn open_direct_stream_with_profile<'a, OpenSocket, OpenSocketFut>(
        &'a self,
        open_socket: OpenSocket,
        profile: TProfile,
    ) -> TransportOpenFuture<'a>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send + 'a;

    fn open_relay_stream_with_profile<'a>(
        &'a self,
        stream: TcpRelayStream,
        profile: TProfile,
    ) -> TransportOpenFuture<'a>;
}

pub fn direct_profiled_stream_opener<'a, TPlan, TProfile, OpenSocket, OpenSocketFut>(
    transport: &'a TPlan,
    open_socket: OpenSocket,
    profile: TProfile,
) -> impl FnOnce() -> TransportOpenFuture<'a>
where
    TPlan: ProfiledTcpStreamTransportPlan<TProfile>,
    TProfile: Send + 'a,
    OpenSocket: FnOnce(&str, u16) -> OpenSocketFut + Send + 'a,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send + 'a,
{
    move || transport.open_direct_stream_with_profile(open_socket, profile)
}

pub fn relay_profiled_stream_opener<'a, TPlan, TProfile>(
    transport: &'a TPlan,
    stream: TcpRelayStream,
    profile: TProfile,
) -> impl FnOnce() -> TransportOpenFuture<'a>
where
    TPlan: ProfiledTcpStreamTransportPlan<TProfile>,
    TProfile: Send + 'a,
{
    move || transport.open_relay_stream_with_profile(stream, profile)
}

pub fn relay_profiled_stream_mapper<'a, TPlan, TProfile>(
    transport: &'a TPlan,
    profile: TProfile,
) -> impl FnOnce(TcpRelayStream) -> TransportOpenFuture<'a>
where
    TPlan: ProfiledTcpStreamTransportPlan<TProfile>,
    TProfile: Send + 'a,
{
    move |stream| transport.open_relay_stream_with_profile(stream, profile)
}
