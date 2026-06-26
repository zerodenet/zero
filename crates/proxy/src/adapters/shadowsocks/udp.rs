use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::{ManagedUdpFlowKind, ProtocolUdpFlowResume};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};
use crate::runtime::Proxy;

impl ShadowsocksAdapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let _ = self;
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        Some(
            crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
                resume.packet_path_cache_key(),
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
        let ResolvedLeafOutbound::Shadowsocks {
            server,
            port,
            password,
            cipher,
            ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let resume =
            shadowsocks::ShadowsocksUdpFlowResume::from_config("", server, *port, cipher, password)
                .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        let codec = Arc::new(resume.packet_path_codec());
        crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
            proxy, server, *port, codec,
        )
        .await
    }

    pub(super) fn udp_datagram_source_impl<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource<'a>> {
        let _ = self;
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        let codec = Arc::new(resume.packet_path_codec());
        Some(crate::runtime::udp_flow::packet_path::udp_datagram_source(
            tag,
            server,
            *port,
            resume.packet_path_cache_key(),
            codec,
        ))
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .map_err(|error| FlowFailure {
            stage: "udp_shadowsocks_resume",
            error: zero_engine::EngineError::Io(std::io::Error::other(error.to_string())),
            upstream: Some((server.to_string(), *port)),
        })?;
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: Some(proxy),
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::shadowsocks(resume),
                payload,
                kind: ManagedUdpFlowKind::Datagram,
                outbound: ManagedUdpOutboundKind::Datagram,
            })
            .await
    }
}
