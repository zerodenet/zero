use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
use crate::runtime::udp_flow::protocol_state::UpstreamAssociationHandler;
use crate::runtime::Proxy;

mod active;
mod model;
mod packet_path;
mod runtime;
mod send;

pub(crate) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    Box::<runtime::Socks5UdpRuntime>::default()
}

impl Socks5Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let _ = self;
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
        let config =
            socks5::Socks5UdpPacketPathConfig::new(tag, server, *port, *username, *password);
        let packet_path = config.packet_path();
        Some(
            crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
                packet_path.cache_key(),
                server,
                *port,
            ),
        )
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
    {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config =
            socks5::Socks5UdpPacketPathConfig::new(tag, server, *port, *username, *password);
        packet_path::build_socks5_packet_path(proxy, tag, server, *port, config.packet_path()).await
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let config =
            socks5::Socks5UdpPacketPathConfig::new(tag, server, *port, *username, *password);
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: Some(proxy),
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: ManagedUdpFlowResume::new(config.flow_resume()),
                payload,
                kind: ManagedUdpFlowKind::RelayStream,
                outbound: ManagedUdpOutboundKind::Relay,
            })
            .await
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream,
            })
    }
}
