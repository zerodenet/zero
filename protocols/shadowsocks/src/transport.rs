//! Shadowsocks UDP socket flow transport helpers.

use std::net::SocketAddr;
use std::sync::Arc;

use zero_core::Address;
use zero_traits::DatagramCodec;
use zero_transport::RuntimeError;

mod inbound;
mod leaf;
mod model;
mod tcp;
mod udp_socket;

pub use inbound::inbound_profile_from_cipher_password;
pub use model::{
    OwnedShadowsocksInboundBindings, OwnedShadowsocksInboundProfile,
    OwnedShadowsocksInboundTcpAcceptor, ShadowsocksManagedDatagramFlowResume,
    ShadowsocksManagedUdpFlowConfig, ShadowsocksManagedUdpFlowPlan,
    ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
    ShadowsocksTransportLeaf, ShadowsocksUdpResponse,
};
pub use tcp::{apply_shadowsocks_tcp_relay_hop, establish_shadowsocks_tcp_connect};
pub use udp_socket::{
    establish_shadowsocks_udp_socket_flow, establish_shadowsocks_udp_socket_flow_with_resume,
    managed_socket_flow_from_resume, ShadowsocksUdpSocketFlow,
};

pub fn udp_flow_resume_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password).flow_resume()
}

pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_carrier_descriptor()
}

pub fn udp_packet_path_carrier_codec_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_carrier_codec()
}

pub fn udp_packet_path_datagram_source_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_datagram_source_build()
}

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
