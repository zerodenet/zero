use std::io;

use crate::runtime::Proxy;
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream};
use zero_engine::EngineError;

/// Relay to fallback: replay captured VLESS header bytes, then relay.
pub(super) async fn relay_fallback<S>(
    proxy: &Proxy,
    fallback_replay: vless::VlessFallbackReplay<S>,
    fallback: &zero_config::FallbackConfig,
) -> Result<(), EngineError>
where
    S: ClientStream,
{
    let mut upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(&fallback.server, fallback.port, proxy.resolver.as_ref())
        .await?;

    let client_stream = fallback_replay.replay_to_upstream(&mut upstream).await?;

    let metered_client = MeteredStream::new(client_stream);
    let metered_upstream = MeteredStream::new(upstream);

    let result =
        relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;

    match result {
        Ok(_) => Ok(()),
        Err(e)
            if e.kind() == io::ErrorKind::NotConnected || e.kind() == io::ErrorKind::BrokenPipe =>
        {
            Ok(())
        }
        Err(e) => Err(EngineError::Io(e)),
    }
}
