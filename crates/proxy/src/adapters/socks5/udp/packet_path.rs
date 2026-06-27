use async_trait::async_trait;
use zero_core::Address;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::establish::{
    establish_shared_packet_path_association, Socks5UdpAssociationEstablishRequest,
};
use super::model::SharedSocks5UdpPacketPathAssociation;
use crate::adapters::common::unreachable_leaf;
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;

pub(crate) struct Socks5PacketPath {
    association: SharedSocks5UdpPacketPathAssociation,
}

#[async_trait]
impl crate::runtime::udp_flow::packet_path::PacketPathCarrier for Socks5PacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.association.send_packet(target, port, payload).await?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association.recv_payload(buf).await
    }
}

pub(super) fn carrier_descriptor(
    leaf: &ResolvedLeafOutbound<'_>,
) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
    let ResolvedLeafOutbound::Socks5 {
        tag,
        server,
        port,
        username,
        password,
    } = leaf
    else {
        return None;
    };
    let config = packet_path_config(tag, server, *port, *username, *password);
    Some(
        crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
            config.packet_path_cache_key(),
            server,
            *port,
        ),
    )
}

pub(super) async fn build(
    adapter: &Socks5Adapter,
    proxy: &Proxy,
    leaf: &ResolvedLeafOutbound<'_>,
) -> Result<std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
{
    let ResolvedLeafOutbound::Socks5 {
        tag,
        server,
        port,
        username,
        password,
    } = leaf
    else {
        return Err(unreachable_leaf(adapter.name(), leaf).error);
    };
    build_socks5_packet_path(
        proxy,
        tag,
        server,
        *port,
        packet_path_config(tag, server, *port, *username, *password)
            .packet_path_association_config(),
    )
    .await
}

pub(crate) async fn build_socks5_packet_path(
    proxy: &Proxy,
    tag: &str,
    server: &str,
    port: u16,
    config: socks5::Socks5UdpAssociationConfig<'_>,
) -> Result<std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
{
    let association =
        establish_shared_packet_path_association(Socks5UdpAssociationEstablishRequest {
            proxy,
            outbound_tag: tag,
            server,
            port,
            config,
            session_id: 0,
        })
        .await?;
    Ok(std::sync::Arc::new(Socks5PacketPath { association }))
}

fn packet_path_config<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    username: Option<&'a str>,
    password: Option<&'a str>,
) -> socks5::Socks5UdpFlowConfig<'a> {
    socks5::Socks5UdpFlowConfig::new(tag, server, port, username, password)
}
