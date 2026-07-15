//! Shadowsocks UDP socket flow transport helpers.

use std::net::SocketAddr;
use zero_transport::RuntimeError;

mod inbound;
mod leaf;
mod model;
mod options;
mod tcp;
mod udp_socket;

pub use model::{
    ShadowsocksInboundBindings, ShadowsocksInboundTcpAcceptor,
    ShadowsocksManagedDatagramFlowResume, ShadowsocksManagedUdpFlowConfig,
    ShadowsocksManagedUdpFlowPlan, ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
    ShadowsocksTransportLeaf, ShadowsocksUdpResponse,
};
pub use options::{ShadowsocksInboundOptionsRef, ShadowsocksOutboundOptionsRef};
pub use tcp::{apply_shadowsocks_tcp_relay_hop, establish_shadowsocks_tcp_connect};
pub use udp_socket::{
    establish_shadowsocks_udp_socket_flow, establish_shadowsocks_udp_socket_flow_with_resume,
    managed_socket_flow_from_resume, ShadowsocksUdpSocketFlow,
};

impl zero_transport::managed_udp::ProtocolManagedDatagramUdpResumeMetadata
    for ShadowsocksManagedDatagramFlowResume
{
    const ESTABLISH_STAGE: &'static str = "ss_establish";
    const MISMATCH_STAGE: &'static str = "udp_shadowsocks_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Shadowsocks UDP flow resume";
}

#[async_trait::async_trait]
impl zero_transport::managed_udp::ProtocolManagedDatagramSocketUdpResumeConnectionOps
    for ShadowsocksManagedDatagramFlowResume
{
    type RawConnection = ShadowsocksUdpSocketFlow;

    const SEND_STAGE: &'static str = "ss_send";
    const RESOLVE_UPSTREAM_MESSAGE: &'static str = "failed to resolve shadowsocks udp upstream";

    fn connector_flow_cache_key(&self, _server: &str, _port: u16) -> String {
        self.socket_flow_spec().into_cache_key()
    }

    async fn open_protocol_connection(
        &self,
        endpoint: SocketAddr,
    ) -> Result<Self::RawConnection, RuntimeError> {
        establish_shadowsocks_udp_socket_flow(
            endpoint,
            self.clone().into_shared_managed_socket_flow_codec(),
        )
        .await
    }
}
