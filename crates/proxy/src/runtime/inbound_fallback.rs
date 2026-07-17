use std::io;
use std::pin::Pin;

use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;
use zero_transport::protocol_inbound_route::FallbackReplayToUpstream;
use zero_transport::OwnedInboundFallbackProfile;

use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream};

pub(crate) async fn relay_recorded_fallback<S, FReplay>(
    services: TcpRuntimeServices,
    fallback: OwnedInboundFallbackProfile,
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
    fallback: OwnedInboundFallbackProfile,
    replay: R,
) -> Result<(), EngineError>
where
    R: FallbackReplayToUpstream + 'static,
{
    relay_recorded_fallback(services, fallback, move |upstream| {
        Box::pin(async move { replay.replay_to_upstream(upstream).await })
    })
    .await
}
