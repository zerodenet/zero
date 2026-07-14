use std::io;
use std::pin::Pin;

use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;
use zero_transport::inbound_route::FallbackReplayToUpstream;
use zero_transport::profile::OwnedInboundFallbackProfile;

use crate::runtime::Proxy;
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream};

pub(crate) async fn relay_recorded_fallback<S, FReplay>(
    proxy: Proxy,
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
    let mut upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(&fallback.server, fallback.port, proxy.resolver.as_ref())
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
    proxy: Proxy,
    fallback: OwnedInboundFallbackProfile,
    replay: R,
) -> Result<(), EngineError>
where
    R: FallbackReplayToUpstream + 'static,
{
    relay_recorded_fallback(proxy, fallback, move |upstream| {
        Box::pin(async move { replay.replay_to_upstream(upstream).await })
    })
    .await
}
