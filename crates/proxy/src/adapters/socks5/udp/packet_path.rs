use async_trait::async_trait;
use zero_core::Address;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::establish::establish_shared_packet_path_carrier;
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
    let spec = socks5::udp_packet_path_spec_from_config(tag, server, *port, *username, *password);
    let descriptor = spec.carrier_descriptor();
    Some(
        crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
            descriptor.cache_key(),
            descriptor.server(),
            descriptor.port(),
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
    let spec = socks5::udp_packet_path_spec_from_config(tag, server, *port, *username, *password);
    build_socks5_packet_path(proxy, spec.carrier_build()).await
}

pub(crate) async fn build_socks5_packet_path(
    proxy: &Proxy,
    carrier: socks5::Socks5UdpPacketPathCarrierBuild,
) -> Result<std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
{
    let association = establish_shared_packet_path_carrier(proxy, carrier).await?;
    Ok(std::sync::Arc::new(Socks5PacketPath { association }))
}
