use async_trait::async_trait;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use super::model::{BoxedSocks5UdpAssociation, SharedSocks5UdpPacketPathAssociation};
use crate::runtime::Proxy;
use zero_engine::EngineError;

pub(super) struct Socks5UdpAssociationEstablishRequest<'a> {
    pub(super) proxy: &'a Proxy,
    pub(super) outbound_tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) config: socks5::Socks5UdpAssociationConfig<'a>,
    pub(super) session_id: u64,
}

#[async_trait]
pub(super) trait Socks5UdpAssociationEstablisher: Send + Sync {
    async fn establish_boxed(
        &self,
        request: Socks5UdpAssociationEstablishRequest<'_>,
    ) -> Result<BoxedSocks5UdpAssociation, EngineError>;

    async fn establish_shared(
        &self,
        request: Socks5UdpAssociationEstablishRequest<'_>,
    ) -> Result<SharedSocks5UdpPacketPathAssociation, EngineError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct DefaultSocks5UdpAssociationEstablisher;

pub(super) fn default_establisher() -> Box<dyn Socks5UdpAssociationEstablisher> {
    Box::new(DefaultSocks5UdpAssociationEstablisher)
}

pub(super) async fn establish_shared_packet_path_association(
    request: Socks5UdpAssociationEstablishRequest<'_>,
) -> Result<SharedSocks5UdpPacketPathAssociation, EngineError> {
    DefaultSocks5UdpAssociationEstablisher
        .establish_shared(request)
        .await
}

#[async_trait]
impl Socks5UdpAssociationEstablisher for DefaultSocks5UdpAssociationEstablisher {
    async fn establish_boxed(
        &self,
        request: Socks5UdpAssociationEstablishRequest<'_>,
    ) -> Result<BoxedSocks5UdpAssociation, EngineError> {
        Ok(Box::new(establish_active(request).await?))
    }

    async fn establish_shared(
        &self,
        request: Socks5UdpAssociationEstablishRequest<'_>,
    ) -> Result<SharedSocks5UdpPacketPathAssociation, EngineError> {
        Ok(std::sync::Arc::new(establish_active(request).await?))
    }
}

async fn establish_active(
    request: Socks5UdpAssociationEstablishRequest<'_>,
) -> Result<ActiveUpstreamSocks5UdpAssociation, EngineError> {
    ActiveUpstreamSocks5UdpAssociation::establish(
        request.proxy,
        request.outbound_tag,
        request.server,
        request.port,
        request.config,
        request.session_id,
    )
    .await
}
