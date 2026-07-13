use std::sync::Arc;

use zero_core::{InboundClientResponse, InboundDatagramUdpRelay, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

#[async_trait::async_trait]
pub trait AuthenticatedQuicInboundProfile: Clone + Send + Sync + 'static {
    type Connection: AuthenticatedQuicInboundConnection;

    async fn accept_authenticated_connection(
        &self,
        connection: quinn::Connection,
    ) -> Result<Self::Connection, EngineError>;
}

#[async_trait::async_trait]
pub trait AuthenticatedQuicInboundConnection: Send + Sync + 'static {
    type Stream: AsyncSocket
        + tokio::io::AsyncRead
        + tokio::io::AsyncWrite
        + Unpin
        + Send
        + Sync
        + 'static;
    type ResponseProtocol: InboundClientResponse<Self::Stream> + Send + Sync + Copy + 'static;
    type UdpRelay: InboundDatagramUdpRelay<Arc<quinn::Connection>> + Send + 'static;

    fn datagram_source(&self) -> Arc<quinn::Connection>;
    fn udp_relay(&self) -> Self::UdpRelay;
    fn response_protocol(&self) -> Self::ResponseProtocol;

    async fn accept_next_tcp_stream(&self) -> Result<Option<(Session, Self::Stream)>, EngineError>;
}
