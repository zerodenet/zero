use std::sync::Arc;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_datagram_connection, ManagedDatagramFlowHandler, ManagedDatagramSender,
    ManagedDatagramSocketFlowConnector, ManagedDatagramSocketFlowManager,
    SharedManagedDatagramUdpConnection,
};
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::{self, ShadowsocksUdpSocketFlow};

struct ShadowsocksManagedDatagramConnector;

pub(super) fn handler() -> Box<dyn ManagedDatagramFlowHandler> {
    Box::new(ManagedDatagramSocketFlowManager::new(
        ShadowsocksManagedDatagramConnector,
        "ss_establish",
        "ss_send",
        "udp_shadowsocks_resume",
        "expected Shadowsocks UDP flow resume",
    ))
}

#[async_trait::async_trait]
impl ManagedDatagramSocketFlowConnector<shadowsocks::ShadowsocksUdpFlowResume>
    for ShadowsocksManagedDatagramConnector
{
    fn flow_cache_key(
        &self,
        resume: &shadowsocks::ShadowsocksUdpFlowResume,
        _endpoint: OutboundEndpoint<'_>,
    ) -> String {
        resume.flow_cache_key()
    }

    async fn establish(
        &self,
        proxy: Option<&crate::runtime::Proxy>,
        endpoint: OutboundEndpoint<'_>,
        resume: shadowsocks::ShadowsocksUdpFlowResume,
        _initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
        let proxy = proxy.ok_or_else(|| {
            EngineError::Io(std::io::Error::other(
                "expected proxy context for Shadowsocks UDP flow",
            ))
        })?;
        let target_addr = proxy
            .protocols
            .direct_connector()
            .resolve_address(
                &endpoint.address(),
                endpoint.port,
                proxy.resolver.as_ref(),
                "failed to resolve shadowsocks udp upstream",
            )
            .await?;
        let flow = Arc::new(
            shadowsocks_transport::establish_shadowsocks_udp_socket_flow(
                target_addr,
                Arc::new(resume.socket_flow_codec()),
            )
            .await?,
        );
        Ok(managed_datagram_connection(
            Arc::new(ShadowsocksDatagramSender { flow: flow.clone() }),
            flow.subscribe(),
            "ss upstream closed",
        ))
    }
}

struct ShadowsocksDatagramSender {
    flow: Arc<ShadowsocksUdpSocketFlow>,
}

#[async_trait::async_trait]
impl ManagedDatagramSender for ShadowsocksDatagramSender {
    async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.flow.send_datagram(target, port, payload).await
    }
}
