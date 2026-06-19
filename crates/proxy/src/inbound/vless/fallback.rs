use std::io;

use tokio::io::{AsyncRead, AsyncWrite};
use zero_platform_tokio::TokioSocket;

use crate::runtime::Proxy;
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream};
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn relay_fallback_no_tls(
        &self,
        client: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
        upstream: TokioSocket,
    ) -> Result<(), EngineError> {
        let metered_client = MeteredStream::new(client);
        let metered_upstream = MeteredStream::new(upstream);
        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;
        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }

    /// Relay to fallback: replay captured VLESS header bytes, then relay.
    pub(crate) async fn relay_fallback<S>(
        &self,
        client_stream: S,
        head: Vec<u8>,
        fallback: &zero_config::FallbackConfig,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut upstream = self
            .protocols
            .direct_outbound
            .connect_host(&fallback.server, fallback.port, self.resolver.as_ref())
            .await?;

        if !head.is_empty() {
            tokio::io::AsyncWriteExt::write_all(&mut upstream, &head).await?;
        }

        let metered_client = MeteredStream::new(client_stream);
        let metered_upstream = MeteredStream::new(upstream);

        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;

        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }
}
