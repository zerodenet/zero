use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::{
    ManagedUdpFlowKind, ProtocolUdpFlowResume, UpstreamAssociationHandler,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};
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
        let packet_path = socks5::Socks5UdpPacketPathConfig {
            tag,
            server,
            port: *port,
            username: *username,
            password: *password,
        };
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
        packet_path::build_socks5_packet_path(proxy, tag, server, *port, username.zip(*password))
            .await
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
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: Some(proxy),
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::new(socks5::Socks5UdpFlowResume::new(
                    *username, *password,
                )),
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
