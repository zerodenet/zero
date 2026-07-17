use crate::RuntimeError;

pub enum TransportInboundBindTarget {
    Tcp,
    #[cfg(feature = "quic")]
    Quic(crate::quic::QuicInbound),
}

#[async_trait::async_trait]
pub trait ProtocolInboundBindPlan {
    async fn bind(&self, listen_addr: &str) -> Result<TransportInboundBindTarget, RuntimeError>;
}
