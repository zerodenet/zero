use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use zero_core::InboundClientResponse;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;
use crate::transport::{relay_bidirectional_metered_throttled, TcpRelayStream};

pub(crate) fn record_tcp_upload(proxy: &Proxy, session_id: u64, bytes: u64) {
    proxy.record_session_inbound_rx(session_id, bytes);
    proxy.record_session_outbound_tx(session_id, bytes);
}

pub(crate) fn record_tcp_download(proxy: &Proxy, session_id: u64, bytes: u64) {
    proxy.record_session_outbound_rx(session_id, bytes);
    proxy.record_session_inbound_tx(session_id, bytes);
}

#[async_trait]
pub(crate) trait InboundProtocol: Send + Sync {
    type ClientStream: AsyncRead + AsyncWrite + Unpin + Send;

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    async fn send_upstream_failure(
        &self,
        client: &mut Self::ClientStream,
    ) -> Result<(), EngineError>;

    async fn relay(
        &self,
        client: Self::ClientStream,
        upstream: TcpRelayStream,
        proxy: &Proxy,
        session_id: u64,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        relay_bidirectional_metered_throttled(
            client,
            upstream,
            |bytes| {
                record_tcp_upload(proxy, session_id, bytes);
            },
            |bytes| {
                record_tcp_download(proxy, session_id, bytes);
            },
            up_bps,
            down_bps,
        )
        .await
        .map(|_| ())
        .map_err(EngineError::Io)
    }
}

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

#[derive(Clone, Copy, Default)]
pub(crate) struct NoClientResponseStreamProtocol<S> {
    _stream: core::marker::PhantomData<fn() -> S>,
}

impl<S> NoClientResponseStreamProtocol<S> {
    pub(crate) const fn new() -> Self {
        Self {
            _stream: core::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<S> InboundProtocol for NoClientResponseStreamProtocol<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    type ClientStream = S;

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct NoClientResponseInboundProtocol;

#[async_trait]
impl InboundProtocol for NoClientResponseInboundProtocol {
    type ClientStream = TcpRelayStream;

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}
