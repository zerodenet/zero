use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use zero_core::InboundClientResponse;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use super::protocol::InboundProtocol;

pub(crate) struct ClientResponseInboundProtocol<P, S> {
    protocol: P,
    _stream: core::marker::PhantomData<fn() -> S>,
}

impl<P, S> ClientResponseInboundProtocol<P, S> {
    pub(crate) const fn new(protocol: P) -> Self {
        Self {
            protocol,
            _stream: core::marker::PhantomData,
        }
    }
}

impl<P, S> Clone for ClientResponseInboundProtocol<P, S>
where
    P: Clone,
{
    fn clone(&self) -> Self {
        Self {
            protocol: self.protocol.clone(),
            _stream: core::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<P, S> InboundProtocol for ClientResponseInboundProtocol<P, S>
where
    P: InboundClientResponse<S> + Send + Sync,
    S: AsyncRead + AsyncWrite + AsyncSocket + Unpin + Send,
{
    type ClientStream = S;

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError> {
        self.protocol
            .send_ok(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError> {
        self.protocol
            .send_blocked(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_upstream_failure(
        &self,
        client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.protocol
            .send_upstream_failure(client)
            .await
            .map_err(EngineError::from)
    }
}
