use std::io;
use std::pin::Pin;

use zero_core::{InboundFallbackReplay, InboundRouteAccept};
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;
use zero_traits::InboundFallbackProfile;

use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream};

#[derive(Debug, Clone)]
pub(crate) struct InboundFallbackTarget {
    server: String,
    port: u16,
}

impl InboundFallbackTarget {
    pub(crate) fn from_profile<T>(profile: &T) -> Self
    where
        T: InboundFallbackProfile + ?Sized,
    {
        Self {
            server: profile.server().to_owned(),
            port: profile.port(),
        }
    }
}

pub(crate) struct PreparedInboundFallback<R> {
    pub(crate) target: InboundFallbackTarget,
    pub(crate) replay: R,
}

pub(crate) type PreparedInboundRouteAccept<R, F> =
    InboundRouteAccept<R, PreparedInboundFallback<F>>;

pub(crate) fn prepare_inbound_route_accept<R, F>(
    result: InboundRouteAccept<R, F>,
    fallback: Option<InboundFallbackTarget>,
) -> Result<PreparedInboundRouteAccept<R, F>, EngineError> {
    match result {
        InboundRouteAccept::Route(route) => Ok(InboundRouteAccept::Route(route)),
        InboundRouteAccept::Fallback(replay) => {
            let target = fallback.ok_or_else(|| {
                EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "protocol requested fallback without a prepared target",
                ))
            })?;
            Ok(InboundRouteAccept::Fallback(PreparedInboundFallback {
                target,
                replay,
            }))
        }
    }
}

pub(crate) async fn relay_recorded_fallback<S, FReplay>(
    services: TcpRuntimeServices,
    fallback: InboundFallbackTarget,
    replay_to_upstream: FReplay,
) -> Result<(), EngineError>
where
    S: ClientStream,
    FReplay: for<'a> FnOnce(
        &'a mut TokioSocket,
    ) -> Pin<
        Box<dyn core::future::Future<Output = Result<S, std::io::Error>> + Send + 'a>,
    >,
{
    let mut upstream = services
        .connect_upstream_owned(fallback.server.clone(), fallback.port)
        .await?;

    let client_stream = replay_to_upstream(&mut upstream).await?;

    let metered_client = MeteredStream::new(client_stream);
    let metered_upstream = MeteredStream::new(upstream);
    match relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await {
        Ok(_) => Ok(()),
        Err(error)
            if error.kind() == io::ErrorKind::NotConnected
                || error.kind() == io::ErrorKind::BrokenPipe =>
        {
            Ok(())
        }
        Err(error) => Err(EngineError::Io(error)),
    }
}

pub(crate) async fn relay_recorded_fallback_replay<R>(
    services: TcpRuntimeServices,
    fallback: InboundFallbackTarget,
    replay: R,
) -> Result<(), EngineError>
where
    R: InboundFallbackReplay + 'static,
    R::Stream: ClientStream,
{
    relay_recorded_fallback(services, fallback, move |upstream| {
        Box::pin(async move { replay.replay_to(upstream).await })
    })
    .await
}
